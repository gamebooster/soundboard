<template>
    <div class="box styled" :class="{'is-collapsed': !this.extended}">
        <div class="is-hidden-tablet has-text-centered extender" :class="{'is-down': this.extended}" @click="toggleExtender">
            <i class="fas fa-chevron-up"></i>
        </div>
        <div class="field">
            <div class="control has-icons-left has-icons-right">
                <span class="icon is-left"><i class="fas fa-volume-down"></i></span>
                <span class="icon is-right"><i class="fas fa-volume-up"></i></span>
                <div class="slider">
                    <b-slider :step="0.05" :min="0.0" :max="1.0" v-model="volume" lazy rounded :custom-formatter="formatVolume"></b-slider>
                </div>
            </div>
        </div>
        <div class="extended-content" :class="{'is-collapsed': !this.extended}">
            <div class="is-hidden-tablet sounds" v-if="activeSounds.length">
                <div v-for="sound in activeSounds" :key="sound.name" class="has-text-centered">
                    <div class="progress-wrapper">
                        <progress class="progress is-primary is-medium" v-if="sound.total_duration > 0" :value="sound.play_duration"
                                  :max="sound.total_duration" size="is-medium"></progress>
                        <div class="progress-value">{{ sound.name }}</div>
                    </div>
                </div>
            </div>
            <b-field>
                <b-input :value="search" @input="val => $emit('search-update', val)" icon="search" rounded type="search"
                         @keyup.enter.native="$event.target.blur()" placeholder="Search all..." />
            </b-field>
            <button class="button is-danger fill-size is-rounded" :class="{'is-outlined': !activeSounds.length}" :disabled="!activeSounds.length" @click="stopSound">Stop</button>
            <div class="">
                <b-select v-model="selectedDevice" rounded class="is-hidden-mobile">
                        <option value="Both">Both</option>
                        <option value="Output">Output</option>
                        <option value="Loop">Loop</option>
                </b-select>
                <div v-if="activeSounds.length" class="is-hidden-mobile">
                    <div v-for="sound in activeSounds" :key="sound.name" class="has-text-centered">
                        <p>{{ sound.name }}</p>
                        <div class="progress-wrapper">
                            <progress class="progress" v-if="sound.total_duration > 0" :value="sound.play_duration"
                                      :max="sound.total_duration"></progress>
                            <div class="progress-value">{{ new Date(sound.play_duration * 1000).toISOString().substr(14, 5)  }} /
                                {{ new Date(sound.total_duration * 1000).toISOString().substr(14, 5)  }}</div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </div>
</template>

<script>
    import axios from "axios";

    export default {
        name: 'ControlBar',
        props: ['activeSounds', 'search'],
        data: function () {
            return {
                volume: 1.0,
                selectedDevice: "Both",
                extended: false,
            };
        },
        watch: {
            volume: (val) => {
                axios.post('/api/sounds/volume', { volume: val });
            }
        },
        methods: {
            formatVolume: val => Math.round(val*100) + '%',
            stopSound() {
                axios.post('/api/sounds/stopall');
                this.$emit('update');
            },
            toggleExtender() {
                this.extended = !this.extended;
                console.log(this.extended);
            },
        }
    }
</script>

<style lang="scss" scoped>
    @import "~bulma/sass/utilities/mixins";
    @import "~bulma";

    .styled {
        box-shadow: 0 0 1em 0.125em rgba(#363636, 0.25);
        z-index: 100;

        .is-collapsed {
            padding-bottom: 0;
        }
    }

    .sounds {
        margin-bottom: 0.5em;

        .progress-wrapper {
            margin-bottom: 0.25rem
        }
    }

    .control.has-icons-right .is-right {
        padding-left: .5em;
    }

    .control.has-icons-left .slider,
    .control.has-icons-right .slider {
        @extend .input;
        border: none;
        box-shadow: none;
    }

    @include until($desktop) {
        .extender {
            margin-top: -1.25rem;
            padding-top: 0.3125rem;
            padding-bottom: 0.625rem;

            &.is-down i {
                transform: rotateX(180deg);
            }

            i {
                transition: transform .3s ease-in-out;
            }
        }

        .progress {
            margin-bottom: .25rem !important;
        }

        .extended-content {
            overflow: hidden;
            height: auto;
            max-height: 33vh;
            transition: max-height .5s ease-in-out;

            &.is-collapsed {
                max-height: 0;
            }
        }
    }


</style>

<style lang="scss">
    .progress-wrapper .progress {
        &::-webkit-progress-value {
            transition-duration: 0.55s !important;
            transition-timing-function: linear !important;
        }
    }
</style>