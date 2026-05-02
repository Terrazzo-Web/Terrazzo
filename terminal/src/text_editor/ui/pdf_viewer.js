let pdfjsLibPromise;

function loadPdfJs() {
    if (!pdfjsLibPromise) {
        pdfjsLibPromise = import("/static/pdfjs/pdf.mjs").then((pdfjsLib) => {
            pdfjsLib.GlobalWorkerOptions.workerSrc = "/static/pdfjs/pdf.worker.mjs";
            return pdfjsLib;
        });
    }
    return pdfjsLibPromise;
}

class PdfJsImpl {
    element;
    loadingTask;
    generation;

    constructor(element, base64) {
        this.element = element;
        this.generation = 0;
        this.set_content(base64);
    }

    destroy() {
        this.generation++;
        if (this.loadingTask) {
            this.loadingTask.destroy();
            this.loadingTask = null;
        }
        this.element.replaceChildren();
    }

    set_content(base64) {
        this.render(base64);
    }

    async render(base64) {
        const generation = ++this.generation;
        if (this.loadingTask) {
            this.loadingTask.destroy();
            this.loadingTask = null;
        }
        this.showStatus("Loading PDF...");

        try {
            const pdfjsLib = await loadPdfJs();

            this.loadingTask = pdfjsLib.getDocument({
                data: base64ToBytes(base64),
            });
            const pdf = await this.loadingTask.promise;
            if (generation !== this.generation) return;

            this.element.replaceChildren();
            for (let pageNumber = 1; pageNumber <= pdf.numPages; pageNumber++) {
                if (generation !== this.generation) return;
                const page = await pdf.getPage(pageNumber);
                await this.renderPage(page, pageNumber);
            }
        } catch (error) {
            if (generation === this.generation) {
                this.showStatus(`Failed to load PDF: ${error}`);
            }
        }
    }

    async renderPage(page, pageNumber) {
        const unscaledViewport = page.getViewport({ scale: 1 });
        const availableWidth = Math.max(this.element.clientWidth - 32, 320);
        const scale = Math.min(Math.max(availableWidth / unscaledViewport.width, 0.5), 2);
        const viewport = page.getViewport({ scale });
        const canvas = document.createElement("canvas");
        canvas.className = "pdf-page";
        canvas.dataset.pageNumber = `${pageNumber}`;
        canvas.width = Math.floor(viewport.width);
        canvas.height = Math.floor(viewport.height);
        canvas.style.width = `${viewport.width}px`;
        canvas.style.height = `${viewport.height}px`;
        canvas.style.maxWidth = "100%";
        canvas.style.backgroundColor = "white";
        canvas.style.boxShadow = "0 2px 12px rgba(0, 0, 0, 0.5)";

        this.element.appendChild(canvas);
        const canvasContext = canvas.getContext("2d");
        await page.render({ canvasContext, viewport }).promise;
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

function base64ToBytes(base64) {
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
        bytes[i] = binary.charCodeAt(i);
    }
    return bytes;
}

export {
    PdfJsImpl
};
