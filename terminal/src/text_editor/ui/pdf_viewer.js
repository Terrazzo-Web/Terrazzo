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

class PdfJsImpl {
    element;
    loadingTask;
    generation;

    constructor(element, data) {
        this.element = element;
        this.generation = 0;
        this.set_content(data);
    }

    destroy() {
        this.generation++;
        if (this.loadingTask) {
            this.loadingTask.destroy();
            this.loadingTask = null;
        }
        this.element.replaceChildren();
    }

    set_content(data) {
        this.render(data);
    }

    async render(data) {
        const generation = ++this.generation;
        if (this.loadingTask) {
            this.loadingTask.destroy();
            this.loadingTask = null;
        }
        this.showStatus("Loading PDF...");

        try {
            const pdfjsLib = await loadPdfJs();

            this.loadingTask = pdfjsLib.getDocument({
                data,
            });
            const pdf = await this.loadingTask.promise;
            if (generation !== this.generation) return;

            this.element.replaceChildren();
            for (let pageNumber = 1; pageNumber <= pdf.numPages; pageNumber++) {
                if (generation !== this.generation) return;
                const page = await pdf.getPage(pageNumber);
                await this.renderPage(pdfjsLib, page, pageNumber);
            }
        } catch (error) {
            if (generation === this.generation) {
                this.showStatus(`Failed to load PDF: ${error}`);
            }
        }
    }

    async renderPage(pdfjsLib, page, pageNumber) {
        const unscaledViewport = page.getViewport({ scale: 1 });
        const availableWidth = Math.max(this.element.clientWidth - 32, 320);
        const scale = Math.min(Math.max(availableWidth / unscaledViewport.width, 0.5), 2);
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
        const renderTask = page.render({ canvasContext, viewport }).promise;
        const textLayerTask = new pdfjsLib.TextLayer({
            textContentSource: page.streamTextContent({ includeMarkedContent: true }),
            container: textLayerElement,
            viewport,
        }).render();

        await Promise.all([renderTask, textLayerTask]);
    }

    totalScaleFactor(viewport) {
        const { pageWidth, pageHeight } = viewport.rawDims;
        const rotated = viewport.rotation % 180 !== 0;
        return rotated ? viewport.width / pageHeight : viewport.width / pageWidth;
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
