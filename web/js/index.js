MobileDragDrop.polyfill({
    // use this to make use of the scroll behaviour
    dragImageTranslateOverride: MobileDragDrop.scrollBehaviourDragImageTranslateOverride
});

var app = new Vue({
    el: "#app",
    data: {
        activeSounds: [],
        soundboards: [],
        volume: 1.0,
        filter: "",
        filterRegex: new RegExp("", "i"),
        soundNames: [],
        matchedSoundNames: [],
        showBottomMenu: true,
        selectedDevice: "Both",
        showStatusModal: false,
        hotkeyEventStream: null,
        registeredHotkeys: new Map()
    },
    created: function () {
        const self = this;
        self.reloadData();
        setInterval(function () {
            self.updatePlayStatus();
        }, 500);
    },
    watch: {
        filter: function (val, oldVal) {
            this.matchedSoundNames = new Map();
            fuzzysort.go(val, this.soundNames, { allowTypo: true, threshold: -25000 }).forEach(s => this.matchedSoundNames.set(s.target, s));
        },
        volume: function (val, oldVal) {
            axios.post("/api/sounds/volume", { volume: val }).catch(function (error) {
                self.showStatusModal = true;
            });
        },
        showBottomMenu: function (val, oldVal) {
            if (val) {
                document.querySelector("nav").classList.remove("hide_bottom_menu");
            } else {
                document.querySelector("nav").classList.add("hide_bottom_menu");
            }
        },
    },
    methods: {
        updatePlayStatus() {
            if (
                getComputedStyle(document.querySelector("#bottom_menu"), null)
                    .display === "none"
            ) {
                this.showBottomMenu = true;
            }
            axios
                .get("/api/sounds/active")
                .then((response) => {
                    this.showStatusModal = false;
                    this.activeSounds = response.data.data.sounds;
                    this.volume = response.data.data.volume;
                })
                .catch((error) => {
                    console.error(error);
                    this.showStatusModal = true;
                });
        },
        reloadData() {
            const self = this;
            axios
                .get("/api/soundboards")
                .then((response) => {
                    self.soundboards = response.data.data;
                    self.soundNames = [];
                    for (let i = 0; i < self.soundboards.length; i++) {
                        const soundboard_id = i;
                        axios
                            .get("/api/soundboards/" + soundboard_id + "/sounds")
                            .then((response) => {
                                self.soundboards[soundboard_id].sounds = response.data.data;
                                self.soundNames = self.soundNames.concat(self.soundboards[soundboard_id].sounds.map(s => s.name));

                                for (const sound of self.soundboards[soundboard_id].sounds) {
                                    if (!sound.hotkey) continue;
                                    this.registerHotkey(sound.hotkey, { soundboard_id: soundboard_id, sound_id: sound.id });
                                }
                            })
                            .catch((error) => {
                                self.showStatusModal = true;
                            });
                    }
                })
                .catch((error) => {
                    self.showStatusModal = true;
                });

            this.hotkeyEvents = new EventSource("/api/hotkeys/events");
            this.hotkeyEvents.onmessage = (event) => {
                let sound_data = this.registeredHotkeys.get(event.data);
                if (sound_data.special === "STOPALL") return this.stopAllSound();
                this.playSound(sound_data.soundboard_id, sound_data.sound_id);
            };

            this.registerHotkey("CTRL-ALT-E", { special: "STOPALL" });
        },
        hideKeyboard() {
            document.activeElement.blur();
        },
        registerHotkey: function (hotkey, eventObject) {
            axios.post("/api/hotkeys", {
                hotkey: hotkey,
            }).then((response) => {
                this.registeredHotkeys.set(hotkey, eventObject);
            }).catch((error) => {
                this.$buefy.toast.open({
                    message: "registerHotkey failed " + response.data.data.hotkey + " " + JSON.stringify(error.response.data.errors),
                    type: "is-danger",
                });
            });
        },
        deregisterHotkey: function (hotkey, eventObject) {
            axios.delete("/api/hotkeys", {
                hotkey: sound.hotkey,
            }).then((response) => {
                this.$buefy.toast.open({
                    message: "deregisterHotkey: " + response.data.data.hotkey,
                    type: "is-success",
                });
                this.registeredHotkeys.set(hotkey, eventObject);
            }).catch((error) => {
                this.$buefy.toast.open({
                    message: "deregisterHotkey failed " + response.data.data.hotkey + " " + error,
                    type: "is-danger",
                });
            });
        },
        playSound: function (soundboard_id, sound_id) {
            axios
                .post(
                    "/api/soundboards/" + soundboard_id + "/sounds/" + sound_id + "/play",
                    {
                        devices: this.selectedDevice,
                    }
                )
                .then((response) => (this.lastRequestAnswer = response.data.data));
        },
        stopSound: function (soundboard_id, sound_id) {
            axios
                .post(
                    "/api/soundboards/" + soundboard_id + "/sounds/" + sound_id + "/stop"
                )
                .then((response) => (this.lastRequestAnswer = response.data.data));
        },
        stopAllSound: function () {
            axios
                .post("/api/sounds/stopall")
                .then((response) => (this.lastRequestAnswer = response.data.data));
        },
        addSoundFromPaste: function (soundboard_id, event) {
            event.preventDefault();

            let text = event.clipboardData.getData("text/plain");
            if (isValidHttpUrl(text)) {
                this.$buefy.dialog.prompt({
                    message: `Please specify a name`,
                    inputAttrs: {
                        placeholder: "e.g. Alarm sound",
                    },
                    trapFocus: true,
                    onConfirm: (value) => {
                        this.addSound(soundboard_id, value, text);
                    },
                });
            } else if (event.clipboardData.files.length > 0) {
                this.addSoundFromFiles(soundboard_id, event.clipboardData.files);
            } else {
                this.$buefy.toast.open({
                    message: "addSoundFromPaste failed: unsupported data",
                    type: "is-danger",
                });
            }
            // console.debug(event.clipboardData);
        },
        soundDragStart: function (soundboard_id, sound_id, event) {
            event.dataTransfer.setData(
                "application/soundboard",
                JSON.stringify({
                    method: "copySound",
                    soundboard_id: soundboard_id,
                    sound_id: sound_id,
                })
            );
        },
        soundboardDragOver: function (soundboard_id, event) {
            let move_data = event.dataTransfer.getData("application/soundboard");
            if (move_data !== "") {
                let data = JSON.parse(move_data);
                if (soundboard_id === data.soundboard_id) return;
            }
            event.target.closest("#soundboard" + soundboard_id).style.background = "#F8F8FF";
            event.preventDefault();
        },
        soundboardDragEnter: function (soundboard_id, event) {
            event.preventDefault();
        },
        soundboardDragLeave: function (soundboard_id, event) {
            event.target.closest("#soundboard" + soundboard_id).style.background = "";
            event.preventDefault();
        },
        addSoundFromDrop: function (soundboard_id, event) {
            event.preventDefault();

            let text = event.dataTransfer.getData("text/plain");
            let move_data = event.dataTransfer.getData("application/soundboard");
            if (isValidHttpUrl(text)) {
                this.$buefy.dialog.prompt({
                    message: `Please specify a name`,
                    inputAttrs: {
                        placeholder: "e.g. Alarm sound",
                    },
                    trapFocus: true,
                    onConfirm: (value) => {
                        this.addSound(soundboard_id, value, text);
                    },
                });
            } else if (move_data !== "") {
                let data = JSON.parse(move_data);
                this.sendCopyExistingSound(
                    soundboard_id,
                    data.soundboard_id,
                    data.sound_id
                );
            } else if (event.dataTransfer.files.length > 0) {
                this.addSoundFromFiles(soundboard_id, event.dataTransfer.files);
            } else {
                this.$buefy.toast.open({
                    message: "addSoundFromDrop failed: unsupported data",
                    type: "is-danger",
                });
            }
            // console.debug(event.dataTransfer);
        },
        addSoundFromFiles: function (soundboard_id, files) {
            let soundboard = this.soundboards[soundboard_id];
            let data = new FormData();

            for (var i = 0; i < files.length; i++) {
                let file = files.item(i);
                data.append(file.name, file, file.name);
            }

            const config = {
                headers: { "content-type": "multipart/form-data" },
            };

            return axios
                .post("/api/soundboards/" + soundboard.id + "/sounds", data, config)
                .then((response) => {
                    response.data.data.forEach((element) => {
                        this.$buefy.toast.open({
                            message: "addSound: " + element.name + " to " + soundboard.name,
                            type: "is-success",
                        });
                        this.soundboards[soundboard_id].sounds.push({
                            name: element.name,
                            hotkey: element.hotkey,
                            id: element.id,
                        });
                    });
                })
                .catch((error) => {
                    this.$buefy.toast.open({
                        duration: 5000,
                        message:
                            `Failed to add sound to soundboard: ` +
                            JSON.stringify(error.response.data.errors),
                        position: "is-top",
                        type: "is-danger",
                    });
                    this.reloadData();
                });
        },
        sendCopyExistingSound: function (
            target_soundboard_id,
            source_soundboard_id,
            source_sound_id
        ) {
            let soundboard = this.soundboards[target_soundboard_id];
            axios
                .post(
                    "/api/soundboards/" + soundboard.id + "/sounds",
                    {
                        source_soundboard_id: source_soundboard_id,
                        source_sound_id: source_sound_id,
                    },
                    { headers: { "x-method": "copy" } }
                )
                .then((response) => {
                    this.$buefy.toast.open({
                        message:
                            "copySound: " +
                            response.data.data.name +
                            " to " +
                            soundboard.name,
                        type: "is-success",
                    });
                    this.soundboards[target_soundboard_id].sounds.push({
                        name: response.data.data.name,
                        hotkey: response.data.data.hotkey,
                        id: response.data.data.id,
                    });
                })
                .catch((error) => {
                    this.$buefy.toast.open({
                        duration: 5000,
                        message:
                            `Failed to add sound to soundboard: ` +
                            JSON.stringify(error.response.data.errors),
                        position: "is-top",
                        type: "is-danger",
                    });
                    this.reloadData();
                });
        },
        addSound: function (soundboard_id, name, path) {
            let soundboard = this.soundboards[soundboard_id];
            axios
                .post(
                    "/api/soundboards/" + soundboard.id + "/sounds",
                    {
                        name: name,
                        hotkey: null,
                        path: path,
                    },
                    { headers: { "x-method": "create" } }
                )
                .then((response) => {
                    this.$buefy.toast.open({
                        message:
                            "addSound: " + response.data.data.name + " to " + soundboard.name,
                        type: "is-success",
                    });
                    this.soundboards[soundboard_id].sounds.push({
                        name: response.data.data.name,
                        hotkey: response.data.data.hotkey,
                        id: response.data.data.id,
                    });
                })
                .catch((error) => {
                    this.$buefy.toast.open({
                        duration: 5000,
                        message:
                            `Failed to add sound to soundboard: ` +
                            JSON.stringify(error.response.data.errors),
                        position: "is-top",
                        type: "is-danger",
                    });
                    this.reloadData();
                });
        },
        deleteSound: function (soundboard_id, sound_id) {
            let soundboard = this.soundboards[soundboard_id];
            let sound = soundboard.sounds[sound_id];

            this.$buefy.dialog.confirm({
                message:
                    "Remove sound <b>" +
                    sound.name +
                    "</b> from soundboard <b>" +
                    soundboard.name +
                    "</b>?",
                type: "is-danger",
                onConfirm: () => {
                    axios
                        .delete("/api/soundboards/" + soundboard.id + "/sounds/" + sound_id)
                        .then((response) => {
                            this.$buefy.toast.open({
                                message:
                                    "deleteSound: " +
                                    soundboard.sounds[sound_id].name +
                                    " from " +
                                    soundboard.name,
                                type: "is-success",
                            });
                            this.soundboards[soundboard_id].sounds.splice(sound_id, 1);
                        })
                        .catch((error) => {
                            this.$buefy.toast.open({
                                duration: 5000,
                                message:
                                    `Failed to delete sound from soundboard: ` +
                                    JSON.stringify(error.response.data.errors),
                                position: "is-top",
                                type: "is-danger",
                            });
                            this.reloadData();
                        });
                },
            });
        },
        updateSoundboard: function (soundboard_id) {
            let soundboard = this.soundboards[soundboard_id];
            axios
                .post("/api/soundboards/" + soundboard.id, {
                    name: soundboard.name,
                    hotkey: soundboard.hotkey,
                    position: soundboard.position,
                })
                .then((response) => {
                    this.$buefy.toast.open({
                        message: "Updated soundboard name to: " + response.data.data.name,
                        type: "is-success",
                    });
                })
                .catch((error) => {
                    this.$buefy.toast.open({
                        duration: 5000,
                        message:
                            `Failed to update soundboard name: ` +
                            JSON.stringify(error.response.data.errors),
                        position: "is-top",
                        type: "is-danger",
                    });
                    this.reloadData();
                });
        },
    },
});

function isValidHttpUrl(string) {
    let url;

    try {
        url = new URL(string);
    } catch (_) {
        return false;
    }

    return url.protocol === "http:" || url.protocol === "https:";
}
