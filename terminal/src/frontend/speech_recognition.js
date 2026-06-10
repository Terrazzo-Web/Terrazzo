export function createSpeechRecognition(onResult, onEnd, onError) {
    const SpeechRecognition = window.SpeechRecognition || window.webkitSpeechRecognition;
    if (!SpeechRecognition) {
        return null;
    }

    const recognition = new SpeechRecognition();
    recognition.interimResults = true;
    recognition.continuous = true;
    recognition.lang = navigator.language || "en-US";

    recognition.onresult = (event) => {
        let transcript = "";
        for (let i = 0; i < event.results.length; i++) {
            const result = event.results[i];
            transcript += Array.from(result).map((r) => r.transcript).join("");
        }
        onResult(transcript);
    };

    recognition.onerror = (event) => {
        const error = event.error || event.message || "speech recognition failed";
        onError(error);
    };

    recognition.onend = () => {
        onEnd();
    };

    return recognition;
}

export function startSpeechRecognition(recognition) {
    if (recognition) {
        recognition.start();
    }
}

export function stopSpeechRecognition(recognition) {
    if (recognition) {
        try {
            recognition.stop();
        } catch (error) {
            // ignore stop errors
        }
    }
}
