var app = new Vue({
    el: '#app',
    data: {
        activeSounds: [],
        soundboards: [],
        volume: 1.0,
        filter: "",
        filterRegex: new RegExp("", "i"),
        showBottomMenu: true,
        selectedDevice: "Both",
        showStatusModal: false
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
            this.filterRegex = new RegExp(val, "i");
        },
        volume: function (val, oldVal) {
            axios
                .post('/api/sounds/volume', { volume: parseFloat(val) })
                .catch(function (error) {
                    self.showStatusModal = true;
                });
        },
        showBottomMenu: function (val, oldVal) {
            if (val) {
                document.querySelector("nav").classList.remove('hide_bottom_menu');
            } else {
                document.querySelector("nav").classList.add('hide_bottom_menu');
            }
        }
    },
    methods: {
        updatePlayStatus() {
            if (getComputedStyle(document.querySelector("#bottom_menu"), null).display === "none") {
                this.showBottomMenu = true;
            }
            axios
                .get('/api/sounds/active')
                .then(response => {
                    this.showStatusModal = false;
                    this.activeSounds = response.data.data.sounds;
                    this.volume = response.data.data.volume;
                }).catch(error => {
                    console.error(error);
                    this.showStatusModal = true;
                });
        },
        reloadData() {
            const self = this;
            axios
                .get('/api/soundboards')
                .then(response => {
                    self.soundboards = response.data.data;
                    for (let i = 0; i < self.soundboards.length; i++) {
                        axios
                            .get('/api/soundboards/' + i + '/sounds')
                            .then(response => {
                                self.soundboards[i].sounds = response.data.data;
                            }).catch(error => {
                                self.showStatusModal = true;
                            });
                    }
                }).catch(error => {
                    self.showStatusModal = true;
                });
        },
        hideKeyboard() {
            document.activeElement.blur();
        },
        playSound: function (soundboard_id, sound_id) {
            axios
                .post('/api/soundboards/' + soundboard_id + '/sounds/' + sound_id + '/play', {
                    devices: this.selectedDevice
                })
                .then(response => (this.lastRequestAnswer = response.data.data))
        },
        stopSound: function (soundboard_id, sound_id) {
            axios
                .post('/api/soundboards/' + soundboard_id + '/sounds/' + sound_id + '/stop')
                .then(response => (this.lastRequestAnswer = response.data.data))
        },
        stopAllSound: function () {
            axios
                .post('/api/sounds/stopall')
                .then(response => (this.lastRequestAnswer = response.data.data))
        },
        updateSoundboard: function (soundboard_id) {
            let soundboard = this.soundboards[soundboard_id];
            axios
                .post('/api/soundboards/' + soundboard.id, {
                    name: soundboard.name,
                    hotkey: soundboard.hotkey,
                    position: soundboard.position
                })
                .then(response => {
                    this.$buefy.toast.open({
                        message: 'Update soundboard to name: ' + response.data.data.name,
                        type: 'is-success'
                    })
                }).catch(error => {
                    this.$buefy.toast.open({
                        duration: 5000,
                        message: `Failed to update soundboard name: ` + JSON.stringify(error.response.data.errors),
                        position: 'is-top',
                        type: 'is-danger'
                    });
                    this.reloadData();
                });
        }
    }
});