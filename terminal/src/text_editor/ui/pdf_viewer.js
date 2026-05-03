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

const MIN_ZOOM = 0.1;
const MAX_ZOOM = 10;
const MIN_ZOOM_SLIDER_VALUE = Math.log10(MIN_ZOOM);
const MAX_ZOOM_SLIDER_VALUE = Math.log10(MAX_ZOOM);
const ZOOM_RENDER_DELAY_MS = 80;

class PdfJsImpl {
    element;
    documentElement;
    loadingTask;
    pdfjsLib;
    pdf;
    generation;
    zoom;
    zoomRenderTimeout;
    zoomAnchor;
    zoomSlider;
    zoomValue;
    activePageTasks;
    onWheel;

    constructor(element, data) {
        this.element = element;
        this.documentElement = null;
        this.generation = 0;
        this.zoom = 1;
        this.zoomRenderTimeout = null;
        this.zoomAnchor = null;
        this.zoomSlider = null;
        this.zoomValue = null;
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
        this.documentElement = null;
        this.zoomSlider = null;
        this.zoomValue = null;
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
        this.documentElement = this.createDocumentElement();
        this.element.replaceChildren();
        this.element.append(this.documentElement, this.createZoomControl());
        this.updateZoomControl();
        for (let pageNumber = 1; pageNumber <= this.pdf.numPages; pageNumber++) {
            if (generation !== this.generation) return;
            const page = await this.pdf.getPage(pageNumber);
            await this.renderPage(generation, page, pageNumber);
        }
        this.restoreZoomAnchor();
    }

    async renderPage(generation, page, pageNumber) {
        const unscaledViewport = page.getViewport({ scale: 1 });
        const availableWidth = Math.max(this.scrollElement.clientWidth - 32, 320);
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
        this.documentElement.appendChild(pageElement);

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

    createDocumentElement() {
        const documentElement = document.createElement("div");
        documentElement.dataset.layer = "pages";
        return documentElement;
    }

    createZoomControl() {
        const control = document.createElement("div");
        control.dataset.control = "zoom";

        const slider = document.createElement("input");
        slider.type = "range";
        slider.min = `${MIN_ZOOM_SLIDER_VALUE}`;
        slider.max = `${MAX_ZOOM_SLIDER_VALUE}`;
        slider.step = "any";
        slider.setAttribute("aria-label", "PDF zoom");
        slider.addEventListener("input", () => {
            this.setZoom(this.zoomFromSliderValue(Number(slider.value)), this.makeCenterZoomAnchor());
        });

        const value = document.createElement("output");
        value.setAttribute("aria-live", "polite");

        control.append(slider, value);
        this.zoomSlider = slider;
        this.zoomValue = value;
        return control;
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
        this.updateZoomControl();
        this.scheduleZoomRender();
    }

    updateZoomControl() {
        const percent = Math.round(this.zoom * 100);
        if (this.zoomSlider) {
            this.zoomSlider.value = `${this.sliderValueFromZoom(this.zoom)}`;
        }
        if (this.zoomValue) {
            this.zoomValue.value = `${percent}%`;
            this.zoomValue.textContent = `${percent}%`;
        }
    }

    zoomFromSliderValue(value) {
        return 10 ** value;
    }

    sliderValueFromZoom(zoom) {
        return Math.log10(Math.min(Math.max(zoom, MIN_ZOOM), MAX_ZOOM));
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
        const scrollElement = this.scrollElement;
        const rect = scrollElement.getBoundingClientRect();
        const pointerY = event.clientY - rect.top;
        const scrollHeight = Math.max(scrollElement.scrollHeight, 1);
        return {
            pointerY,
            ratio: (scrollElement.scrollTop + pointerY) / scrollHeight,
        };
    }

    makeCenterZoomAnchor() {
        const scrollElement = this.scrollElement;
        const pointerY = scrollElement.clientHeight / 2;
        const scrollHeight = Math.max(scrollElement.scrollHeight, 1);
        return {
            pointerY,
            ratio: (scrollElement.scrollTop + pointerY) / scrollHeight,
        };
    }

    restoreZoomAnchor() {
        const anchor = this.zoomAnchor;
        this.zoomAnchor = null;
        if (!anchor) return;
        const scrollElement = this.scrollElement;
        scrollElement.scrollTop = anchor.ratio * scrollElement.scrollHeight - anchor.pointerY;
    }

    get scrollElement() {
        return this.documentElement ?? this.element;
    }

    showStatus(message) {
        this.documentElement = null;
        this.zoomSlider = null;
        this.zoomValue = null;
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
