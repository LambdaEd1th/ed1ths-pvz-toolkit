(() => {
    const VERSION = 3;
    const BAR_COUNT = 56;
    const PLAYBACK_RATES = [0.75, 1, 1.25, 1.5, 2];
    const existing = window.wemPlayer;
    if (existing?.version === VERSION) {
        requestAnimationFrame(() => existing.bind());
        return;
    }
    existing?.destroy?.();
    window.wemWaveform?.destroy?.();

    let audio = null;
    let canvas = null;
    let controls = null;
    let stateLabel = null;
    let context2d = null;
    let graph = null;
    let frequencyData = null;
    let resizeObserver = null;
    let themeObserver = null;
    let themeMediaQuery = null;
    let themeFrame = 0;
    let seekInput = null;
    let currentTimeOutput = null;
    let durationOutput = null;
    let volumeInput = null;
    let rateValue = null;
    let playButton = null;
    let muteButton = null;
    let frame = 0;
    let frameCount = 0;
    let lastFrameAt = 0;
    let lastPeak = 0;
    let buffering = false;
    let lastVolume = 1;
    const levels = new Float32Array(BAR_COUNT);

    const idleLevel = (index) => {
        const first = Math.sin(index * 0.72) * 0.035;
        const second = Math.sin(index * 0.23 + 1.4) * 0.025;
        return 0.075 + Math.abs(first + second);
    };

    const formatTime = (seconds) => {
        if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
        const whole = Math.floor(seconds);
        const hours = Math.floor(whole / 3600);
        const minutes = Math.floor((whole % 3600) / 60);
        const remaining = whole % 60;
        if (hours > 0) {
            return `${hours}:${String(minutes).padStart(2, "0")}:${String(remaining).padStart(2, "0")}`;
        }
        return `${minutes}:${String(remaining).padStart(2, "0")}`;
    };

    const clampTime = (value) => {
        if (!audio) return 0;
        const duration = Number.isFinite(audio.duration) ? audio.duration : 0;
        return Math.max(0, Math.min(duration, value));
    };

    const sizeCanvas = () => {
        if (!canvas) return null;
        const bounds = canvas.getBoundingClientRect();
        const ratio = Math.min(window.devicePixelRatio || 1, 2);
        const width = Math.max(1, Math.round(bounds.width * ratio));
        const height = Math.max(1, Math.round(bounds.height * ratio));
        if (canvas.width !== width || canvas.height !== height) {
            canvas.width = width;
            canvas.height = height;
        }
        context2d?.setTransform(ratio, 0, 0, ratio, 0, 0);
        return { width: bounds.width, height: bounds.height };
    };

    const roundedBar = (x, y, width, height, radius) => {
        if (!context2d) return;
        context2d.beginPath();
        if (typeof context2d.roundRect === "function") {
            context2d.roundRect(x, y, width, height, radius);
        } else {
            context2d.rect(x, y, width, height);
        }
        context2d.fill();
    };

    const drawWaveform = () => {
        if (!canvas || !context2d) return;
        const size = sizeCanvas();
        if (!size) return;
        context2d.clearRect(0, 0, size.width, size.height);

        const gap = Math.max(2, Math.min(6, size.width / 165));
        const available = Math.max(1, size.width - gap * (BAR_COUNT - 1));
        const barWidth = Math.max(2, Math.min(5, available / BAR_COUNT));
        const totalWidth = barWidth * BAR_COUNT + gap * (BAR_COUNT - 1);
        const startX = (size.width - totalWidth) / 2;
        const centerY = size.height / 2;
        const color = getComputedStyle(canvas).color || "rgb(66, 207, 192)";

        context2d.fillStyle = color;
        context2d.shadowColor = color;
        context2d.shadowBlur = Math.min(14, size.height * 0.07);
        for (let index = 0; index < BAR_COUNT; index += 1) {
            const amount = Math.max(0.045, Math.min(1, levels[index]));
            const height = Math.max(4, amount * size.height * 0.82);
            context2d.globalAlpha = 0.42 + amount * 0.58;
            roundedBar(
                startX + index * (barWidth + gap),
                centerY - height / 2,
                barWidth,
                height,
                barWidth / 2,
            );
        }
        context2d.globalAlpha = 1;
        context2d.shadowBlur = 0;
        frameCount += 1;
    };

    const settleWaveform = () => {
        let moving = false;
        for (let index = 0; index < BAR_COUNT; index += 1) {
            const idle = idleLevel(index);
            const next = levels[index] + (idle - levels[index]) * 0.16;
            moving ||= Math.abs(next - idle) > 0.004;
            levels[index] = next;
        }
        lastPeak *= 0.84;
        drawWaveform();
        return moving;
    };

    const analyseWaveform = () => {
        if (!graph || !frequencyData) return;
        graph.analyser.getByteFrequencyData(frequencyData);
        let peak = 0;
        for (let index = 0; index < BAR_COUNT; index += 1) {
            const position = index / Math.max(1, BAR_COUNT - 1);
            const bin = Math.min(
                frequencyData.length - 2,
                Math.round(Math.pow(position, 1.55) * frequencyData.length * 0.68),
            );
            const raw =
                (frequencyData[bin] * 0.62 +
                    frequencyData[bin + 1] * 0.26 +
                    frequencyData[Math.max(0, bin - 1)] * 0.12) /
                255;
            const target = Math.max(0.045, Math.pow(raw, 1.22));
            const speed = (target > levels[index] ? 58 : 20) / 100;
            levels[index] += (target - levels[index]) * speed;
            peak = Math.max(peak, levels[index]);
        }
        lastPeak = peak;
        drawWaveform();
    };

    const bufferedPercent = (duration) => {
        if (!audio || duration <= 0 || audio.buffered.length === 0) return 0;
        let end = 0;
        for (let index = 0; index < audio.buffered.length; index += 1) {
            end = Math.max(end, audio.buffered.end(index));
        }
        return Math.max(0, Math.min(100, (end / duration) * 100));
    };

    const updateTimeline = () => {
        if (!audio || !controls || !seekInput || !currentTimeOutput || !durationOutput) return;
        const duration = Number.isFinite(audio.duration) ? audio.duration : 0;
        const current = duration > 0 ? clampTime(audio.currentTime) : 0;
        const played = duration > 0 ? (current / duration) * 100 : 0;
        const buffered = bufferedPercent(duration);

        currentTimeOutput.textContent = formatTime(current);
        durationOutput.textContent = formatTime(duration);
        seekInput.value = String(Math.round(played * 10));
        seekInput.disabled = duration <= 0;
        seekInput.setAttribute(
            "aria-valuetext",
            `${formatTime(current)} / ${formatTime(duration)}`,
        );
        controls.style.setProperty("--wem-played", `${played}%`);
        controls.style.setProperty("--wem-buffered", `${Math.max(played, buffered)}%`);
    };

    const updateVolume = () => {
        if (!audio || !controls || !volumeInput || !muteButton) return;
        const volume = Math.round(audio.volume * 100);
        const muted = audio.muted || volume === 0;
        if (!audio.muted && volume > 0) lastVolume = audio.volume;
        volumeInput.value = String(volume);
        volumeInput.setAttribute("aria-valuetext", muted ? "静音" : `${volume}%`);
        controls.style.setProperty("--wem-volume", `${volume}%`);
        controls.dataset.muted = String(muted);
        muteButton.title = muted ? "取消静音" : "静音";
        muteButton.setAttribute("aria-label", muteButton.title);
    };

    const formatRate = (rate) => {
        const value = Number.isInteger(rate) ? String(rate) : String(rate).replace(/0+$/, "");
        return `${value}×`;
    };

    const updateRate = () => {
        if (!audio || !rateValue) return;
        const label = formatRate(audio.playbackRate);
        rateValue.textContent = label;
        const button = rateValue.closest("button");
        button?.setAttribute("aria-label", `播放速度 ${label}`);
        if (button) button.title = `播放速度 ${label}，点击切换`;
    };

    const updateMediaState = () => {
        if (!audio || !controls || !stateLabel || !playButton) return;
        const playing = !audio.paused && !audio.ended;
        controls.dataset.playing = String(playing);
        controls.dataset.buffering = String(buffering);
        let label = "已暂停";
        if (buffering && playing) {
            label = "缓冲中";
        } else if (playing) {
            label = "播放中";
        } else if (audio.ended) {
            label = "已结束";
        } else if (audio.readyState < 1) {
            label = "正在载入";
        } else if (audio.currentTime === 0) {
            label = "就绪";
        }
        stateLabel.textContent = label;
        playButton.title = playing ? "暂停" : "播放";
        playButton.setAttribute("aria-label", playButton.title);
    };

    const updateControls = () => {
        updateTimeline();
        updateVolume();
        updateRate();
        updateMediaState();
    };

    const renderFrame = (time) => {
        frame = 0;
        if (!audio) return;
        const playing = !audio.paused && !audio.ended;
        updateTimeline();
        if (!playing) {
            if (settleWaveform()) frame = requestAnimationFrame(renderFrame);
            return;
        }
        if (time - lastFrameAt >= 30) {
            lastFrameAt = time;
            analyseWaveform();
        }
        frame = requestAnimationFrame(renderFrame);
    };

    const schedule = () => {
        if (frame) cancelAnimationFrame(frame);
        frame = requestAnimationFrame(renderFrame);
    };

    const ensureGraph = () => {
        if (!audio) return false;
        if (audio.__wemPlayerGraph) {
            graph = audio.__wemPlayerGraph;
            frequencyData = new Uint8Array(graph.analyser.frequencyBinCount);
            return true;
        }
        const AudioContext = window.AudioContext || window.webkitAudioContext;
        if (!AudioContext) return false;
        try {
            const audioContext = new AudioContext();
            const source = audioContext.createMediaElementSource(audio);
            const analyser = audioContext.createAnalyser();
            analyser.fftSize = 256;
            analyser.minDecibels = -88;
            analyser.maxDecibels = -18;
            analyser.smoothingTimeConstant = 0.76;
            source.connect(analyser);
            analyser.connect(audioContext.destination);
            graph = { audioContext, source, analyser };
            audio.__wemPlayerGraph = graph;
            frequencyData = new Uint8Array(analyser.frequencyBinCount);
            return true;
        } catch (error) {
            console.warn("WEM player analyser is unavailable", error);
            return false;
        }
    };

    const togglePlayback = async () => {
        if (!audio) return;
        if (!audio.paused && !audio.ended) {
            audio.pause();
            return;
        }
        if (audio.ended) audio.currentTime = 0;
        try {
            await audio.play();
        } catch (error) {
            buffering = false;
            updateMediaState();
            console.warn("WEM playback could not start", error);
        }
    };

    const skip = (seconds) => {
        if (!audio || !Number.isFinite(audio.duration)) return;
        audio.currentTime = clampTime(audio.currentTime + seconds);
        updateTimeline();
    };

    const cycleRate = () => {
        if (!audio) return;
        const current = audio.playbackRate;
        let index = PLAYBACK_RATES.findIndex((rate) => Math.abs(rate - current) < 0.01);
        if (index < 0) index = 0;
        audio.playbackRate = PLAYBACK_RATES[(index + 1) % PLAYBACK_RATES.length];
        updateRate();
    };

    const toggleMute = () => {
        if (!audio) return;
        if (audio.muted || audio.volume === 0) {
            if (audio.volume === 0) audio.volume = Math.max(0.1, lastVolume);
            audio.muted = false;
        } else {
            audio.muted = true;
        }
        updateVolume();
    };

    const onControlClick = (event) => {
        const button = event.target?.closest?.("button[data-wem-action]");
        if (!button || !controls?.contains(button)) return;
        const action = button.dataset.wemAction;
        if (action === "play") togglePlayback();
        if (action === "mute") toggleMute();
        if (action === "rate") cycleRate();
    };

    const onControlInput = (event) => {
        const input = event.target;
        const action = input?.dataset?.wemAction;
        if (!audio) return;
        if (action === "seek" && Number.isFinite(audio.duration)) {
            audio.currentTime = clampTime((Number(input.value) / 1000) * audio.duration);
            updateTimeline();
        }
        if (action === "volume") {
            const volume = Math.max(0, Math.min(1, Number(input.value) / 100));
            audio.volume = volume;
            audio.muted = volume === 0;
            updateVolume();
        }
    };

    const onControlKeyDown = (event) => {
        if (!controls || event.target !== controls) return;
        const key = event.key.toLowerCase();
        if (key === " " || key === "k") {
            event.preventDefault();
            togglePlayback();
        } else if (key === "arrowleft") {
            event.preventDefault();
            skip(-5);
        } else if (key === "arrowright") {
            event.preventDefault();
            skip(5);
        } else if (key === "m") {
            event.preventDefault();
            toggleMute();
        } else if (key === "home" && Number.isFinite(audio?.duration)) {
            event.preventDefault();
            audio.currentTime = 0;
            updateTimeline();
        } else if (key === "end" && Number.isFinite(audio?.duration)) {
            event.preventDefault();
            audio.currentTime = audio.duration;
            updateTimeline();
        }
    };

    const onPlay = async () => {
        buffering = false;
        if (ensureGraph()) {
            try {
                await graph.audioContext.resume();
            } catch (_error) {}
        }
        updateMediaState();
        schedule();
    };

    const onPause = () => {
        buffering = false;
        updateControls();
        schedule();
    };

    const onWaiting = () => {
        buffering = true;
        updateMediaState();
    };

    const onPlaying = () => {
        buffering = false;
        updateMediaState();
        schedule();
    };

    const onLoaded = () => {
        for (let index = 0; index < BAR_COUNT; index += 1) {
            levels[index] = idleLevel(index);
        }
        drawWaveform();
        updateControls();
    };

    const onResize = () => drawWaveform();

    const onThemeChange = () => {
        if (themeFrame) cancelAnimationFrame(themeFrame);
        themeFrame = requestAnimationFrame(() => {
            themeFrame = 0;
            drawWaveform();
        });
    };

    const observeTheme = () => {
        if (typeof MutationObserver === "function") {
            themeObserver = new MutationObserver(onThemeChange);
            const shell = canvas?.closest?.(".tk-shell");
            themeObserver.observe(document.documentElement, {
                attributes: true,
                attributeFilter: ["class", "style"],
            });
            if (shell && shell !== document.documentElement) {
                themeObserver.observe(shell, {
                    attributes: true,
                    attributeFilter: ["class", "style"],
                });
            }
        }

        themeMediaQuery = window.matchMedia?.("(prefers-color-scheme: dark)") ?? null;
        if (typeof themeMediaQuery?.addEventListener === "function") {
            themeMediaQuery.addEventListener("change", onThemeChange);
        } else {
            themeMediaQuery?.addListener?.(onThemeChange);
        }
    };

    const stopObservingTheme = () => {
        themeObserver?.disconnect();
        themeObserver = null;
        if (typeof themeMediaQuery?.removeEventListener === "function") {
            themeMediaQuery.removeEventListener("change", onThemeChange);
        } else {
            themeMediaQuery?.removeListener?.(onThemeChange);
        }
        themeMediaQuery = null;
        if (themeFrame) cancelAnimationFrame(themeFrame);
        themeFrame = 0;
    };

    const unbind = () => {
        if (frame) cancelAnimationFrame(frame);
        frame = 0;
        stopObservingTheme();
        if (audio) {
            audio.removeEventListener("play", onPlay);
            audio.removeEventListener("pause", onPause);
            audio.removeEventListener("ended", onPause);
            audio.removeEventListener("waiting", onWaiting);
            audio.removeEventListener("playing", onPlaying);
            audio.removeEventListener("loadedmetadata", onLoaded);
            audio.removeEventListener("durationchange", updateTimeline);
            audio.removeEventListener("timeupdate", updateTimeline);
            audio.removeEventListener("progress", updateTimeline);
            audio.removeEventListener("volumechange", updateVolume);
            audio.removeEventListener("ratechange", updateRate);
            audio.removeEventListener("seeked", updateTimeline);
            if (audio.__wemPlayerGraph === graph) {
                audio.__wemPlayerGraph = null;
            }
        }
        controls?.removeEventListener("click", onControlClick);
        controls?.removeEventListener("input", onControlInput);
        controls?.removeEventListener("keydown", onControlKeyDown);
        graph?.audioContext?.close?.();
        resizeObserver?.disconnect();
        resizeObserver = null;
        window.removeEventListener("resize", onResize);
        audio = null;
        canvas = null;
        controls = null;
        stateLabel = null;
        context2d = null;
        graph = null;
        frequencyData = null;
        seekInput = null;
        currentTimeOutput = null;
        durationOutput = null;
        volumeInput = null;
        rateValue = null;
        playButton = null;
        muteButton = null;
        buffering = false;
    };

    const bind = () => {
        const root = document.querySelector('[data-tool="wem"] .wem-player-panel');
        const nextAudio = root?.querySelector(".wem-audio-player");
        const nextCanvas = root?.querySelector(".wem-waveform-canvas");
        const nextControls = root?.querySelector(".wem-player-controls");
        if (!nextAudio || !nextCanvas || !nextControls) {
            unbind();
            return;
        }
        if (audio === nextAudio && canvas === nextCanvas && controls === nextControls) {
            drawWaveform();
            updateControls();
            return;
        }

        unbind();
        audio = nextAudio;
        canvas = nextCanvas;
        controls = nextControls;
        stateLabel = root.querySelector(".wem-player-state-label");
        context2d = canvas.getContext("2d", { alpha: true });
        seekInput = controls.querySelector('[data-wem-action="seek"]');
        currentTimeOutput = controls.querySelector(".wem-current-time");
        durationOutput = controls.querySelector(".wem-duration-time");
        volumeInput = controls.querySelector('[data-wem-action="volume"]');
        rateValue = controls.querySelector(".wem-rate-value");
        playButton = controls.querySelector('[data-wem-action="play"]');
        muteButton = controls.querySelector('[data-wem-action="mute"]');

        audio.addEventListener("play", onPlay);
        audio.addEventListener("pause", onPause);
        audio.addEventListener("ended", onPause);
        audio.addEventListener("waiting", onWaiting);
        audio.addEventListener("playing", onPlaying);
        audio.addEventListener("loadedmetadata", onLoaded);
        audio.addEventListener("durationchange", updateTimeline);
        audio.addEventListener("timeupdate", updateTimeline);
        audio.addEventListener("progress", updateTimeline);
        audio.addEventListener("volumechange", updateVolume);
        audio.addEventListener("ratechange", updateRate);
        audio.addEventListener("seeked", updateTimeline);
        controls.addEventListener("click", onControlClick);
        controls.addEventListener("input", onControlInput);
        controls.addEventListener("keydown", onControlKeyDown);

        if (typeof ResizeObserver === "function") {
            resizeObserver = new ResizeObserver(onResize);
            resizeObserver.observe(canvas);
        } else {
            window.addEventListener("resize", onResize);
        }
        observeTheme();
        onLoaded();
        if (!audio.paused) onPlay();
    };

    const suspend = () => {
        if (audio && !audio.paused) audio.pause();
        graph?.audioContext?.suspend?.();
        if (frame) cancelAnimationFrame(frame);
        frame = 0;
        buffering = false;
        updateControls();
    };

    window.wemPlayer = {
        version: VERSION,
        bind,
        unbind,
        suspend,
        destroy: unbind,
        diagnostics: () => ({
            active: Boolean(audio && !audio.paused && !audio.ended && frame),
            audioContextState: graph?.audioContext?.state || "uninitialized",
            frameCount,
            lastPeak,
            currentTime: audio?.currentTime || 0,
            duration: Number.isFinite(audio?.duration) ? audio.duration : 0,
            playbackRate: audio?.playbackRate || 1,
            volume: audio?.volume ?? 1,
            muted: audio?.muted || false,
        }),
    };
    requestAnimationFrame(bind);
})();
