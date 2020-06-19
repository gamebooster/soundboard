var app = new Vue({
    el: '#app',
    data: {
        activeSounds: [],
        soundboards: [],
        lastRequestAnswer: "",
        filter: "",
        volume: 1.0,
        filterRegex: new RegExp("", "i"),
        show_bottom_menu: true,
        selected_device: "Both",
        show_status_modal: false
    },
    created: function () {
        const self = this;
        setInterval(function () {
            if (getComputedStyle(document.querySelector("#bottom_menu"), null).display === "none") {
                self.show_bottom_menu = true;
            }
            axios
                .get('/api/sounds/active')
                .then(response => {
                    self.show_status_modal = false;
                    self.activeSounds = response.data.data;
                }).catch(function (error) {
                    console.log(error);
                    self.show_status_modal = true;
                });
        }, 500);
        axios
            .get('/api/soundboards')
            .then(response => {
                self.soundboards = response.data.data;
                for (let i = 0; i < self.soundboards.length; i++) {
                    axios
                        .get('/api/soundboards/' + i + '/sounds')
                        .then(response => {
                            self.soundboards[i].sounds = response.data.data;
                        }).catch(function (error) {
                            console.log(error);
                            self.show_status_modal = true;
                        });
                }
            }).catch(function (error) {
                console.log(error);
                self.show_status_modal = true;
            });
    },
    watch: {
        filter: function (val, oldVal) {
            this.filterRegex = new RegExp(val, "i");
        },
        volume: function (val, oldVal) {
            axios
                .post('/api/sounds/volume', { volume: parseFloat(val) })
                .then(response => (this.lastRequestAnswer = response.data.data))
        },
        show_bottom_menu: function (val, oldVal) {
            if (val) {
                document.querySelector("nav").classList.remove('hide_bottom_menu');
            } else {
                document.querySelector("nav").classList.add('hide_bottom_menu');
            }
        }
    },
    methods: {
        hideKeyboard() {
            document.activeElement.blur();
        },
        playSound: function (soundboard_id, sound_id) {
            axios
                .post('/api/soundboards/' + soundboard_id + '/sounds/' + sound_id + '/play', {
                    devices: this.selected_device
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
        }
    }
})