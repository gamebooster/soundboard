<template>
    <div @dragover="dragEnter" @drop="dragDrop" @dragleave="dragLeave" :class="{'is-highlighted': draggedOver}">
        <div class="level">
            <div class="level-left">
                <div class="level-item">
                    <h3 class="title">{{ data.name }}</h3>
                </div>
                <div class="level-item">
                    <p class="subtitle">({{ data.sounds ? data.sounds.length : 0 }} sounds)</p>
                </div>
                <div class="level-item buttons" v-if="selectedSounds.length">
                    <button @click="selectedSoundsChangeName" class="button is-outlined is-warning"
                            :disabled="selectedSounds.length !== 1"><i class="fas fa-edit"></i></button>
                    <button @click="selectedSoundsDelete" class="button is-outlined is-danger"><i
                            class="fas fa-trash"></i></button>
                </div>
            </div>
            <div class="level-right">
                <b-field class="level-item">
                    <b-input v-model="search" icon="search" rounded type="search"
                             @keyup.enter.native="$event.target.blur()" placeholder="Search this board..."/>
                </b-field>
            </div>
        </div>
        <div class="buttons" v-if="filteredSounds.length">
            <button v-for="sound in filteredSounds" class="button is-success"
                    :class="{'is-danger': soundNames.includes(sound.name), 'is-warning is-outlined': selectedSounds.includes(sound.id)}"
                    :key="sound.id"
                    @click="soundNames.includes(sound.name) ? stopSound(sound.id) : playSound(sound.id)" @click.right="e => selectSound(sound.id, e)">{{ sound.name }}
            </button>
        </div>
        <p v-else>No sounds here :(</p>
    </div>
</template>

<script>
    import axios from "axios";

    export default {
        name: "Soundboard",
        props: ['id', 'activeSounds', 'search'],
        data: function () {
            return {
                data: {},
                draggedOver: false,
                selectedSounds: [],
            }
        },
        computed: {
            filteredSounds() {
                if (this.data && this.data.sounds) {
                    return this.data.sounds.filter(s => {
                        return !this.search || s.name.toLowerCase().includes(this.search.toLowerCase());
                    });
                } else {
                    return [];
                }
            },
            soundNames() {
                return this.activeSounds.length ? this.activeSounds.map(val => val.name) : [];
            }
        },
        methods: {
            fetchData() {
                axios.get('/api/soundboards/' + this.id)
                    .then(resp => {
                        this.data = resp.data.data
                    });
            },
            playSound(id) {
                axios.post('/api/soundboards/' + this.id + '/sounds/' + id + '/play', {devices: "Both"})
                    .then(() => {
                        this.$emit('update');
                    });
            },
            stopSound(id) {
                axios.post('/api/soundboards/' + this.id + '/sounds/' + id + '/stop')
                    .then(() => {
                        this.$emit('update');
                    });
            },
            selectSound(soundId, evt) {
                evt.preventDefault();
                if (!this.selectedSounds.includes(soundId)) {
                    this.selectedSounds.push(soundId);
                } else {
                    this.selectedSounds.splice(this.selectedSounds.indexOf(soundId), 1);
                }
                document.activeElement.blur();
            },
            selectedSoundsChangeName() {
                let name = this.data.sounds.find(e => e.id = this.selectedSounds[0]).name;
                this.$buefy.dialog.prompt({
                    message: `What do you want to rename the sound <b>"${name}"</b> to?`,
                    inputAttrs: {
                        type: 'text',
                        value: name,
                    },
                    onConfirm: () => {
                        this.$buefy.toast.open({
                            message: 'This feature will be available in the near future...',
                            type: 'is-warning',
                        });
                    }
                });
            },
            selectedSoundsDelete() {
                this.$buefy.dialog.confirm({
                    message: `Do you really want to delete the selected sounds?`,
                    type: 'is-danger',
                    icon: 'trash',
                    onConfirm: () => {
                        axios.delete('/api/soundboards/' + this.id + '/sounds/' + this.selectedSounds[0])
                            .then(() => {
                                this.$buefy.toast.open({
                                    message: 'You have deleted the sound.',
                                    type: 'is-success',
                                });
                                this.fetchData();
                                this.selectedSounds = [];
                            });
                    }
                });
            },
            dragEnter(evt) {
                this.draggedOver = true;
                evt.preventDefault();
            },
            dragLeave(evt) {
                this.draggedOver = false;
                evt.preventDefault();
            },
            dragDrop(evt) {
                this.draggedOver = false;
                evt.preventDefault();

                if (evt.dataTransfer.files.length > 0) {
                    let data = new FormData();

                    for (let file of evt.dataTransfer.files) {
                        data.append(file.name, file, file.name);
                    }

                    axios.post('/api/soundboards/' + this.id + '/sounds', data,
                        {headers: {'content-type': 'multipart/form-data'}})
                        .then(resp => {
                            for (let elem of resp.data.data) {
                                this.$buefy.toast.open({
                                    message: 'Added sound ' + elem.name + ' to ' + this.data.name,
                                    type: 'is-success',
                                });
                            }
                            this.reloadData();
                        });
                }
            }
        },
        mounted() {
            this.fetchData();
        }
    }
</script>

<style scoped lang="scss">
    @import url("https://use.fontawesome.com/releases/v5.2.0/css/all.css");
    @import "~bulma/sass/utilities/all";

    .buttons {
        .button {
            flex-grow: 1;
            flex-shrink: 1;
        }
    }

    .is-highlighted {
        @extend .has-shadow !optional;
        border: 3px dashed $primary;
        position: relative;

        &::before {
            content: 'Drop the file here, to upload.';
            text-align: center;
            padding-top: 4rem;
            position: absolute;
            top: 0;
            left: 0;
            height: 100%;
            width: 100%;
            background-color: rgba(255, 255, 255, 0.5);
            z-index: 50;
        }
    }
</style>