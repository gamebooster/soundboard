<template>
    <div class="container is-fluid">
        <div class="modal is-active" v-if="slowMode" id="status_modal">
            <div class="modal-background"></div>
            <div class="notification is-danger">
                <p class="title is-5 center">Lost connection to soundboard server</p>
            </div>
        </div>
        <section class="hero is-dark">
            <div class="hero-body">
                <h1 class="title">
                    soundboard
                </h1>
                <div class="buttons">
                    <button v-for="soundboard in soundboards" :key="soundboard.id" class="button is-rounded"
                            :class="{'is-primary': soundboard.active}"
                    @click="toggleBoard(soundboard.id)">{{ soundboard.name }}
                    </button>
                </div>
            </div>
        </section>
        <div class="main">
<!--            <div class="box" v-for="soundboard in activeSoundboards" :key="soundboard.id">-->
            <Soundboard class="box" v-for="soundboard in activeSoundboards" :key="soundboard.id" :id="soundboard.id" :active-sounds="activeSounds" :search="search" @update="updatePlayStatus"></Soundboard>
<!--            </div>-->
        </div>
        <control-bar class="container is-4 control-bar" :selected-device="selectedDevice" :active-sounds="activeSounds"
                     :search="search" @search-update="searchUpdate"></control-bar>
    </div>
</template>

<script>
    import axios from 'axios';
    import '../assets/index.css';
    import Soundboard from "./Soundboard";
    import ControlBar from "./ControlBar";

    axios.defaults.baseURL = 'http://localhost:3030';
    export default {
        name: 'HelloWorld',
        props: {
            msg: String
        },
        components: {
            Soundboard,
            ControlBar
        },
        data: function () {
            return {
                activeSounds: [],
                soundboards: [],
                volume: 1.0,
                filter: "",
                filterRegex: new RegExp("", "i"),
                showBottomMenu: true,
                selectedDevice: "Both",
                search: "",
                slowMode: false,
                updateInterval: -1,
            }
        },
        created: function () {
            this.reloadData();
            this.updateInterval = setInterval(this.updatePlayStatus, 500);
        },
        destroyed() {
            clearInterval(this.updateInterval);
        },
        computed: {
            activeSoundboards: function () {
                return this.search ? this.soundboards : this.soundboards.filter(val => val.active);
            }
        },
        watch: {
            slowMode: function (val, oldVal) {
                if (val && !oldVal) {
                    clearInterval(this.updateInterval);
                    this.updateInterval = setInterval(this.updatePlayStatus, 5000);
                } else if (!val && oldVal) {
                    clearInterval(this.updateInterval);
                    this.updateInterval = setInterval(this.updatePlayStatus, 500);
                }
            }
        },
        methods: {
            searchUpdate(val) {
                console.log(val);
                this.search = val;
            },
            toggleBoard(boardId) {
                this.$set(this.soundboards[boardId], "active", !this.soundboards[boardId].active);
            },
            updatePlayStatus() {
                axios.get('/api/sounds/active')
                .then(response => {
                    this.slowMode = false;
                    this.showStatusModal = false;
                    this.activeSounds = response.data.data.sounds;
                    this.volume = response.data.data.volume;
                }).catch(error => {
                    console.error(error);
                    this.slowMode = true;
                    this.showStatusModal = true;
                });
            },
            reloadData() {
                axios.get('/api/soundboards')
                .then(response => {
                    this.soundboards = response.data.data;
                }).catch(() => {
                    this.showStatusModal = true;
                });
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
                            message: 'Updated soundboard name to: ' + response.data.data.name,
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
    }
</script>

<!-- Add "scoped" attribute to limit CSS to this component only -->
<style lang="scss" scoped>
    @import "~bulma/sass/utilities/mixins";

    .main {
        padding-bottom: 32px;
    }

    // Put control bar at the bottom until the screen is wide enough
    @include until($desktop) {
        .control-bar {
            position: fixed;
            bottom: 0;
            left: 0;
            width: calc(100% - 2*16px);
            /*min-height: calc(1/6 * 100vh - 2*16px);*/
            margin: 16px 16px 0;
            border-bottom-left-radius: 0;
            border-bottom-right-radius: 0;
        }

        .main {
            padding-bottom: calc(1/6 * 100vh);
        }

        .buttons {
            margin: 0 -16px;
            overflow: scroll;
            flex-wrap: nowrap;

            &::before {
                padding-left: 16px;
                content: '';
            }
        }
    }

    @include desktop {
        .control-bar {
            position: fixed;
            top: 0;
            right: 0;
            height: calc(100vh - 2*32px);
            width: calc(2/6 * 100vw - 2*32px);
            margin: 32px;
        }

        .main {
            width:  calc(4/6 * 100vw - 2*32px);
        }
    }

    @include widescreen {
        .control-bar {
            width: calc(1/6 * 100vw - 2*32px);
        }
        .main {
            width: calc(5/6 * 100vw - 2*32px);
        }
    }
</style>
