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
    zoomRenderStartedAt;
    zoomAnchor;
    zoomControl;
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
        this.zoomRenderStartedAt = 0;
        this.zoomAnchor = null;
        this.zoomControl = null;
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
        this.zoomControl = null;
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
            this.zoomRenderStartedAt = window.performance.now();
        } catch (error) {
            if (generation === this.generation) {
                this.showStatus(`Failed to load PDF: ${error}`);
            }
        }
    }

    async renderDocument(generation, preserveZoomControl = false) {
        this.cancelPageTasks();
        const previousDocumentElement = this.documentElement;
        const documentElement = this.createDocumentElement();
        this.documentElement = documentElement;
        const zoomControl = preserveZoomControl && this.zoomControl
            ? this.zoomControl
            : this.createZoomControl();

        if (preserveZoomControl && previousDocumentElement?.parentElement === this.element) {
            previousDocumentElement.replaceWith(documentElement);
            if (zoomControl.parentElement !== this.element) {
                this.element.appendChild(zoomControl);
            }
        } else {
            this.element.replaceChildren(documentElement, zoomControl);
        }
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

        const annotationLayerElement = document.createElement("div");
        annotationLayerElement.dataset.layer = "annotations";

        pageElement.append(canvas, textLayerElement, annotationLayerElement);
        this.documentElement.appendChild(pageElement);

        const canvasContext = canvas.getContext("2d");
        const renderTask = page.render({ canvasContext, viewport });
        const textLayer = new this.pdfjsLib.TextLayer({
            textContentSource: page.streamTextContent({ includeMarkedContent: true }),
            container: textLayerElement,
            viewport,
        });
        const annotations = page
            .getAnnotations({ intent: "display" })
            .then((annotations) => this.renderLinks(annotationLayerElement, annotations, viewport));
        const tasks = { renderTask, textLayer };
        this.activePageTasks.add(tasks);

        try {
            await Promise.all([renderTask.promise, textLayer.render(), annotations]);
        } catch (error) {
            if (generation === this.generation) {
                throw error;
            }
        } finally {
            this.activePageTasks.delete(tasks);
        }
    }

    renderLinks(layerElement, annotations, viewport) {
        const linkType = this.pdfjsLib.AnnotationType.LINK;
        for (const annotation of annotations) {
            if (annotation.annotationType !== linkType || !annotation.rect) {
                continue;
            }

            const externalHref = annotation.url ?? annotation.unsafeUrl;
            const href = externalHref ?? this.destinationHref(annotation.dest);
            if (!href) {
                continue;
            }

            const rect = viewport.convertToViewportRectangle(annotation.rect);
            const left = Math.min(rect[0], rect[2]);
            const top = Math.min(rect[1], rect[3]);
            const width = Math.abs(rect[0] - rect[2]);
            const height = Math.abs(rect[1] - rect[3]);
            if (!width || !height) {
                continue;
            }

            const link = document.createElement("a");
            link.href = href;
            link.title = href;
            link.style.left = `${left}px`;
            link.style.top = `${top}px`;
            link.style.width = `${width}px`;
            link.style.height = `${height}px`;
            if (externalHref) {
                link.target = "_blank";
                link.rel = "noopener noreferrer";
            } else {
                link.addEventListener("click", (event) => {
                    event.preventDefault();
                    this.scrollToDestination(annotation.dest);
                });
            }
            layerElement.appendChild(link);
        }
    }

    async scrollToDestination(destination) {
        const explicitDestination = typeof destination === "string"
            ? await this.pdf.getDestination(destination)
            : destination;
        if (!explicitDestination?.length) {
            return;
        }

        const pageNumber = await this.destinationPageNumber(explicitDestination[0]);
        if (!pageNumber) {
            return;
        }

        const pageElement = this.documentElement?.querySelector(`div[data-page-number="${pageNumber}"]`);
        if (!pageElement) {
            return;
        }

        const scrollTop = await this.destinationScrollTop(pageElement, pageNumber, explicitDestination);
        this.scrollElement.scrollTo({ top: scrollTop, behavior: "smooth" });
    }

    async destinationPageNumber(pageReference) {
        if (Number.isInteger(pageReference)) {
            return pageReference + 1;
        }

        try {
            return await this.pdf.getPageIndex(pageReference) + 1;
        } catch (_) {
            return null;
        }
    }

    async destinationScrollTop(pageElement, pageNumber, explicitDestination) {
        const destinationKind = explicitDestination[1]?.name;
        const destinationTop = explicitDestination[3];
        if ((destinationKind !== "XYZ" && destinationKind !== "FitH" && destinationKind !== "FitBH")
            || typeof destinationTop !== "number") {
            return pageElement.offsetTop;
        }

        const page = await this.pdf.getPage(pageNumber);
        const unscaledViewport = page.getViewport({ scale: 1 });
        const scale = pageElement.getBoundingClientRect().width / unscaledViewport.width;
        const viewport = page.getViewport({ scale });
        return pageElement.offsetTop + viewport.convertToViewportPoint(0, destinationTop)[1];
    }

    destinationHref(destination) {
        if (!destination) {
            return null;
        }
        if (typeof destination === "string") {
            return `#${encodeURIComponent(destination)}`;
        }
        return `#${encodeURIComponent(JSON.stringify(destination))}`;
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
        this.zoomControl = control;
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
        const now = window.performance.now();
        const delay = this.zoomRenderStartedAt
            ? Math.max(0, ZOOM_RENDER_DELAY_MS - (now - this.zoomRenderStartedAt))
            : ZOOM_RENDER_DELAY_MS;
        this.zoomRenderTimeout = window.setTimeout(() => {
            this.zoomRenderTimeout = null;
            if (!this.pdf || !this.pdfjsLib) {
                return;
            }
            this.zoomRenderStartedAt = window.performance.now();
            const generation = ++this.generation;
            this.renderDocument(generation, true).catch((error) => {
                if (generation === this.generation) {
                    this.showStatus(`Failed to zoom PDF: ${error}`);
                }
            });
        }, delay);
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
        this.zoomControl = null;
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
