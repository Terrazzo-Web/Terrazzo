let pdfjsLibPromise;

function loadPdfJs() {
    if (!pdfjsLibPromise) {
        pdfjsLibPromise = import("/static/pdfjs/pdf.min.mjs").then((pdfjsLib) => {
            pdfjsLib.GlobalWorkerOptions.workerSrc = "/static/pdfjs/pdf.worker.min.mjs";
            return pdfjsLib;
        });
    }
    return pdfjsLibPromise;
}

const MIN_ZOOM = 0.25;
const MAX_ZOOM = 6;
const ZOOM_RENDER_DELAY_MS = 80;

class PdfJsImpl {
    element;
    loadingTask;
    pdfjsLib;
    pdf;
    generation;
    zoom;
    zoomRenderTimeout;
    zoomAnchor;
    activePageTasks;
    onWheel;

    constructor(element, data) {
        this.element = element;
        this.generation = 0;
        this.zoom = 1;
        this.zoomRenderTimeout = null;
        this.zoomAnchor = null;
        this.activePageTasks = new Set();
        this.onWheel = (event) => this.handleWheel(event);
        this.element.addEventListener("wheel", this.onWheel, { passive: false });
        this.set_content(data);
    }

    destroy() {
        this.generation++;
        this.clearZoomRender();
        this.cancelPageTasks();
        this.element.removeEventListener("wheel", this.onWheel);
        if (this.loadingTask) {
            this.loadingTask.destroy();
            this.loadingTask = null;
        }
        this.pdfjsLib = null;
        this.pdf = null;
        this.element.replaceChildren();
    }

    set_content(data) {
        this.zoom = 1;
        this.render(data);
    }

    async render(data) {
        const generation = ++this.generation;
        this.clearZoomRender();
        this.cancelPageTasks();
        if (this.loadingTask) {
            this.loadingTask.destroy();
            this.loadingTask = null;
        }
        this.pdfjsLib = null;
        this.pdf = null;
        this.showStatus("Loading PDF...");

        try {
            const pdfjsLib = await loadPdfJs();
            if (generation !== this.generation) return;

            this.loadingTask = pdfjsLib.getDocument({
                data,
            });
            const pdf = await this.loadingTask.promise;
            if (generation !== this.generation) return;

            this.pdfjsLib = pdfjsLib;
            this.pdf = pdf;
            await this.renderDocument(generation);
        } catch (error) {
            if (generation === this.generation) {
                this.showStatus(`Failed to load PDF: ${error}`);
            }
        }
    }

    async renderDocument(generation) {
        this.cancelPageTasks();
        this.element.replaceChildren();
        for (let pageNumber = 1; pageNumber <= this.pdf.numPages; pageNumber++) {
            if (generation !== this.generation) return;
            const page = await this.pdf.getPage(pageNumber);
            await this.renderPage(generation, page, pageNumber);
        }
        this.restoreZoomAnchor();
    }

    async renderPage(generation, page, pageNumber) {
        const unscaledViewport = page.getViewport({ scale: 1 });
        const availableWidth = Math.max(this.element.clientWidth - 32, 320);
        const scale = Math.min(Math.max(availableWidth / unscaledViewport.width, 0.5), 2) * this.zoom;
        const viewport = page.getViewport({ scale });

        const pageElement = document.createElement("div");
        pageElement.dataset.pageNumber = `${pageNumber}`;
        pageElement.style.width = `${viewport.width}px`;
        pageElement.style.height = `${viewport.height}px`;
        pageElement.style.setProperty("--total-scale-factor", `${this.totalScaleFactor(viewport)}`);

        const canvas = document.createElement("canvas");
        canvas.dataset.pageNumber = `${pageNumber}`;
        canvas.width = Math.floor(viewport.width);
        canvas.height = Math.floor(viewport.height);
        canvas.style.width = `${viewport.width}px`;
        canvas.style.height = `${viewport.height}px`;

        const textLayerElement = document.createElement("div");
        textLayerElement.dataset.layer = "text";

        pageElement.append(canvas, textLayerElement);
        this.element.appendChild(pageElement);

        const canvasContext = canvas.getContext("2d");
        const renderTask = page.render({ canvasContext, viewport });
        const textLayer = new this.pdfjsLib.TextLayer({
            textContentSource: page.streamTextContent({ includeMarkedContent: true }),
            container: textLayerElement,
            viewport,
        });
        const tasks = { renderTask, textLayer };
        this.activePageTasks.add(tasks);

        try {
            await Promise.all([renderTask.promise, textLayer.render()]);
        } catch (error) {
            if (generation === this.generation) {
                throw error;
            }
        } finally {
            this.activePageTasks.delete(tasks);
        }
    }

    totalScaleFactor(viewport) {
        const { pageWidth, pageHeight } = viewport.rawDims;
        const rotated = viewport.rotation % 180 !== 0;
        return rotated ? viewport.width / pageHeight : viewport.width / pageWidth;
    }

    handleWheel(event) {
        if (!this.pdf || !this.pdfjsLib || !(event.ctrlKey || event.metaKey)) {
            return;
        }
        event.preventDefault();
        event.stopPropagation();

        const factor = Math.max(0.8, Math.min(1.25, Math.exp(-event.deltaY * 0.002)));
        this.setZoom(this.zoom * factor, this.makeZoomAnchor(event));
    }

    setZoom(zoom, anchor) {
        const nextZoom = Math.min(Math.max(zoom, MIN_ZOOM), MAX_ZOOM);
        if (Math.abs(nextZoom - this.zoom) < 0.001) {
            return;
        }
        this.zoom = nextZoom;
        this.zoomAnchor = anchor;
        this.scheduleZoomRender();
    }

    scheduleZoomRender() {
        this.clearZoomRender();
        this.zoomRenderTimeout = window.setTimeout(() => {
            this.zoomRenderTimeout = null;
            if (!this.pdf || !this.pdfjsLib) {
                return;
            }
            const generation = ++this.generation;
            this.renderDocument(generation).catch((error) => {
                if (generation === this.generation) {
                    this.showStatus(`Failed to zoom PDF: ${error}`);
                }
            });
        }, ZOOM_RENDER_DELAY_MS);
    }

    clearZoomRender() {
        if (this.zoomRenderTimeout) {
            window.clearTimeout(this.zoomRenderTimeout);
            this.zoomRenderTimeout = null;
        }
    }

    cancelPageTasks() {
        for (const { renderTask, textLayer } of this.activePageTasks) {
            renderTask.cancel();
            textLayer.cancel();
        }
        this.activePageTasks.clear();
    }

    makeZoomAnchor(event) {
        const rect = this.element.getBoundingClientRect();
        const pointerY = event.clientY - rect.top;
        const scrollHeight = Math.max(this.element.scrollHeight, 1);
        return {
            pointerY,
            ratio: (this.element.scrollTop + pointerY) / scrollHeight,
        };
    }

    restoreZoomAnchor() {
        const anchor = this.zoomAnchor;
        this.zoomAnchor = null;
        if (!anchor) return;
        this.element.scrollTop = anchor.ratio * this.element.scrollHeight - anchor.pointerY;
    }

    showStatus(message) {
        const status = document.createElement("div");
        status.className = "pdf-status";
        status.textContent = message;
        status.style.alignSelf = "stretch";
        status.style.padding = "var(--padding)";
        this.element.replaceChildren(status);
    }
}

export {
    PdfJsImpl
};
