MobileDragDrop.polyfill({
  // use this to make use of the scroll behaviour
  dragImageTranslateOverride:
    MobileDragDrop.scrollBehaviourDragImageTranslateOverride,
});

const ModalForm = {
  props: ['initialName', 'initialHotkey', 'initialSource'],
  data: function () {
    return {
      name: this.initialName,
      hotkey: this.initialHotkey,
      source: JSON.stringify(this.initialSource)
    };
  },
  template: `
        <form @submit.prevent="$emit('submit', {name: name, hotkey: hotkey, source: source});  $parent.close();">
            <div class="modal-card">
                <header class="modal-card-head">
                    <p class="modal-card-title">Edit sound</p>
                </header>
                <section class="modal-card-body">
                    <b-field label="Name" label-position="on-border">
                        <b-input :value="name" v-model="name" required>
                        </b-input>
                    </b-field>

                    <b-field label="Source" label-position="on-border">
                        <b-input :value="source" v-model="source" type="textarea" required>
                        </b-input>
                    </b-field>

                    <b-field label="Hotkey" label-position="on-border">
                        <b-input :value="hotkey" v-model="hotkey">
                        </b-input>
                    </b-field>
                </section>
                <footer class="modal-card-foot" style="display: block;">
                  <nav class="level">
                    <!-- Left side -->
                    <div class="level-left">
                        <div class="level-item">
                            <button class="button is-danger" type="button" @click="$emit('delete'); $parent.close();">Delete</button>
                        </div>
                    </div>
                    <div class="level-right">
                        <div class="level-item">
                            <button class="button" type="button" @click="$parent.close()">Cancel</button>
                        </div>
                        <div class="level-item">
                            <button class="button is-primary">Submit</button>
                        </div>
                    </div>
                  </nav>
                </footer>
            </div>
        </form>
    `
}

var app = new Vue({
  el: '#app',
  components: { ModalForm },
  data: {
    activeSounds: [],
    soundboards: [],
    soundNames: [],
    matchedSoundNames: [],
    filter: '',
    volume: 1.0,
    selectedDevice: 'Both',
    showBottomMenu: true,
    showStatusModal: false,
    showLoadingModal: true,
    registeredHotkeys: new Map(),
  },
  created: function () {
    this.reloadData(window.location.search.includes('reload') ? true : false);
    this.createEventSources();
  },
  watch: {
    filter: function (val, oldVal) {
      this.matchedSoundNames = new Map();
      fuzzysort.go(val, this.soundNames, { allowTypo: true, threshold: -25000 })
        .forEach((s) => this.matchedSoundNames.set(s.target, s));
    },
    volume: function (val, oldVal) {
      axios.post('/api/sounds/volume', { volume: val }).catch(function (error) {
        self.showStatusModal = true;
      });
    },
    showBottomMenu: function (val, oldVal) {
      if (val) {
        document.querySelector('nav').classList.remove('hide_bottom_menu');
      } else {
        document.querySelector('nav').classList.add('hide_bottom_menu');
      }
    },
  },
  methods: {
    // sortChanged(soundboard, value) {
    //   if (value === 'Name Ascending') {
    //     soundboard.sounds.sort((a, b) => a.name.localeCompare(b.name));
    //   } else if (value === 'Name Descending') {
    //     soundboard.sounds.sort((a, b) => -a.name.localeCompare(b.name));
    //   } else if (value === 'Hotkey') {
    //     soundboard.sounds.sort(
    //       (a, b) => a.hotkey ? (a.hotkey).localeCompare(b.hotkey) :
    //         (b.hotkey ? 1 : a.id - b.id));
    //   } else {
    //     soundboard.sounds.sort((a, b) => a.id.localeCompare(b.id));
    //   }
    // },
    createEventSources() {
      this.soundEvents = new EventSource('/api/sounds/events');
      this.soundEvents.onmessage = (event) => {
        let play_data = JSON.parse(event.data);
        this.activeSounds = play_data.sounds;
        this.volume = play_data.volume;
        if (this.showStatusModal) {
          this.registeredHotkeys.clear();
          this.reloadData();
        }
        this.showStatusModal = false;
        if (getComputedStyle(document.querySelector('#bottom_menu'), null)
          .display === 'none') {
          this.showBottomMenu = true;
        }
      };
      this.soundEvents.onerror = (err) => {
        this.showStatusModal = true;
        this.soundEvents.close();
        setTimeout(() => {
          this.createEventSources();
        }, 1000);
      };

      this.hotkeyEvents = new EventSource('/api/hotkeys/events');
      this.hotkeyEvents.onmessage = (event) => {
        let sound_data = this.registeredHotkeys.get(event.data);
        if (sound_data.special === 'STOPALL') return this.stopAllSound();
        this.playSound(sound_data.soundboard_id, sound_data.sound_id);
      };

      this.hotkeyEvents.onerror = (err) => {
        this.hotkeyEvents.close();
        this.showStatusModal = true;
      };
    },
    reloadData(reload_from_disk) {
      const self = this;
      self.showLoadingModal = true;
      axios
        .get(
          reload_from_disk ? '/api/soundboards?reload' : '/api/soundboards')
        .then((response) => {
          response.data.data.sounds = [];
          this.soundboards = response.data.data;
          this.soundNames = [];
          let requests = [];
          for (soundboard of this.soundboards) {
            requests.push(
              axios.get('/api/soundboards/' + soundboard.id));
          }
          Promise.all(requests)
            .then((sounds_responses) => {
              let soundboard_index = 0;
              for (const response of sounds_responses) {
                let soundboard = self.soundboards[soundboard_index];
                soundboard.sounds = response.data.data.sounds;
                for (const sound of soundboard.sounds) {
                  self.soundNames.push(sound.name);
                  if (!sound.hotkey) continue;
                  self.registerHotkey(sound.hotkey, {
                    soundboard_id: soundboard.id,
                    sound_id: sound.id,
                  });
                }
                soundboard.order = 'Index';
                soundboard_index++;
              }
              self.showLoadingModal = false;
              self.registerHotkey('CTRL-ALT-E', { special: 'STOPALL' });
            })
            .catch((errors) => {
              self.showStatusModal = true;
            });
        })
        .catch((error) => {
          self.showStatusModal = true;
        });
    },
    hideKeyboard() {
      document.activeElement.blur();
    },
    showResponseError(text, err) {
      this.showWarning(
        text + ' error: ' +
        (err.response ? JSON.stringify(err.response.data.errors) :
          JSON.stringify(err)));
    },
    showWarning(text) {
      this.$buefy.toast.open(
        { message: text, type: 'is-danger', duration: 5000, queue: false });
    },
    showSuccess(text) {
      this.$buefy.toast.open({ message: text, type: 'is-success', queue: false });
    },
    registerHotkey: function (hotkey, eventObject) {
      if (this.registeredHotkeys.has(hotkey)) {
        return;
        // this.showWarning(  'registerHotkey failed: ' + hotkey + ' already
        // registered!');
      }

      axios
        .post('/api/hotkeys', {
          hotkey: hotkey,
        })
        .then((response) => {
          this.registeredHotkeys.set(hotkey, eventObject);
        })
        .catch((error) => {
          this.showResponseError('registerHotkey', error);
        });
    },
    deregisterHotkey: function (hotkey) {
      axios
        .delete('/api/hotkeys', {
          data: {
            hotkey: hotkey,
          }
        })
        .then((response) => {
          this.showSuccess('deregisterHotkey: ' + response.data.data.hotkey);
          this.registeredHotkeys.delete(hotkey);
        })
        .catch((error) => {
          this.showResponseError('deregisterHotkey', error);
        });
    },
    playSound: function (soundboard_id, sound_id) {
      axios
        .post(
          '/api/soundboards/' + soundboard_id + '/sounds/' + sound_id +
          '/play',
          {
            devices: this.selectedDevice,
          })
        .then((response) => (this.lastRequestAnswer = response.data.data));
    },
    stopSound: function (soundboard_id, sound_id) {
      axios
        .post(
          '/api/soundboards/' + soundboard_id + '/sounds/' + sound_id +
          '/stop')
        .then((response) => (this.lastRequestAnswer = response.data.data));
    },
    stopAllSound: function () {
      axios.post('/api/sounds/stopall')
        .then((response) => (this.lastRequestAnswer = response.data.data));
    },
    addSoundFromPaste: function (soundboard_id, event) {
      event.preventDefault();

      let text = event.clipboardData.getData('text/plain');
      if (isValidHttpUrl(text)) {
        this.$buefy.dialog.prompt({
          message: `Please specify a name`,
          inputAttrs: {
            placeholder: 'e.g. Alarm sound',
          },
          trapFocus: true,
          onConfirm: (value) => {
            let soundboard = this.soundboards.find((s) => s.id === soundboard_id);
            var last_id = soundboard.sounds[soundboard.sounds.length - 1].id;
            this.addSound(soundboard_id, last_id, value, { http: { url: text } });
          },
        });
      } else if (event.clipboardData.files.length > 0) {
        this.addSoundFromFiles(
          soundboard_id, sound_id, event.clipboardData.files);
      } else {
        this.showWarning('addSoundFromPaste failed: unsupported data');
      }
    },
    soundDragStart: function (soundboard_id, sound_id, event) {
      var data = {
        soundboard_id: soundboard_id,
        sound_id: sound_id,
      };
      this.dragData = data;
      event.dataTransfer.setData(
        'application/soundboard', JSON.stringify(data));
    },
    soundboardDragOver: function (soundboard_id, event) {
      event.preventDefault();
    },
    soundboardDragEnter: function (soundboard_id, event) {
      event.preventDefault();
    },
    soundboardDragLeave: function (soundboard_id, event) {
      event.preventDefault();
    },
    soundDragOver: function (soundboard_id, sound_id, event) {
      if (this.dragData && soundboard_id === this.dragData.soundboard_id &&
        sound_id == this.dragData.sound_id) {
        return;
      }
      event.preventDefault();
    },
    soundDragEnter: function (soundboard_id, sound_id, event) {
      event.preventDefault();
    },
    soundDragLeave: function (soundboard_id, sound_id, event) {
      event.preventDefault();
    },
    addSoundFromDrop: function (soundboard_id, sound_id, event) {
      event.stopPropagation();
      event.preventDefault();
      this.dragData = null;

      let text = event.dataTransfer.getData('text/plain');
      let move_data = event.dataTransfer.getData('application/soundboard');
      if (isValidHttpUrl(text)) {
        this.$buefy.dialog.prompt({
          message: `Please specify a name`,
          inputAttrs: {
            placeholder: 'e.g. Alarm sound',
          },
          trapFocus: true,
          onConfirm: (value) => {
            this.addSound(soundboard_id, sound_id, value, { http: { url: text } });
          },
        });
      } else if (move_data !== '') {
        let data = JSON.parse(move_data);
        this.sendCopyExistingSound(
          soundboard_id, sound_id, data.soundboard_id, data.sound_id,
          soundboard_id == data.soundboard_id ? true : false);

      } else if (event.dataTransfer.files.length > 0) {
        this.addSoundFromFiles(
          soundboard_id, sound_id, event.dataTransfer.files);
      } else {
        this.showWarning('addSoundFromDrop failed: unsupported data');
      }
    },
    updateSoundboardIds: function (soundboard_id) {
      // let soundboard = this.soundboards[soundboard_id];
      // for (let i = 0; i < soundboard.sounds.length; i++) {
      //   soundboard.sounds[i].id = i;
      // }
    },
    addSoundFromFiles: function (soundboard_id, sound_id, files) {
      let soundboard = this.soundboards.find((s) => s.id === soundboard_id);

      let data = new FormData();

      for (var i = 0; i < files.length; i++) {
        let file = files.item(i);
        data.append(file.name, file, file.name);
      }

      const config = {
        headers: { 'content-type': 'multipart/form-data' },
      };

      return axios
        .post(
          '/api/soundboards/' + soundboard.id + '/sounds/' + sound_id, data,
          config)
        .then((response) => {
          let sounds = response.data.data;
          sounds.forEach((element) => {
            this.showSuccess(
              'addSound: ' + element.name + ' to ' + soundboard.name);
          });

          soundboard.sounds.splice(soundboard.sounds.findIndex((s) => s.id === sound_id) + 1, 0, ...response.data.data);
        })
        .catch((error) => {
          this.showResponseError('addSoundFromFiles', error);
          this.reloadData();
        });
    },
    sendCopyExistingSound: function (
      target_soundboard_id, target_sound_id, source_soundboard_id,
      source_sound_id, delete_source_sound) {
      let soundboard = this.soundboards.find((s) => s.id === target_soundboard_id);

      axios
        .post(
          '/api/soundboards/' + soundboard.id + '/sounds/' +
          target_sound_id,
          {
            source_soundboard_id: source_soundboard_id,
            source_sound_id: source_sound_id,
          },
          { headers: { 'x-method': 'copy' } })
        .then((response) => {
          let sound = response.data.data;
          this.showSuccess(
            'copySound: ' + sound.name + ' to ' + soundboard.name);

          let index = soundboard.sounds.findIndex((s) => s.id === target_sound_id);
          soundboard.sounds.splice(index + 1, 0, sound);

          if (delete_source_sound) {
            this.deleteSound(source_soundboard_id, source_sound_id, true);
          }
        })
        .catch((error) => {
          this.showResponseError('sendCopyExistingSound', error);
          this.reloadData();
        });
    },
    editSound: function (soundboard_id, sound_id) {
      let soundboard = this.soundboards.find((s) => s.id === soundboard_id);
      if (!soundboard) return;
      let sound = soundboard.sounds.find((s) => s.id === sound_id);
      if (!sound) return;

      let props = {
        'initialName': sound.name,
        'initialHotkey': sound.hotkey,
        'initialSource': sound.source
      };

      this.$buefy.modal.open({
        parent: this,
        component: ModalForm,
        hasModalCard: true,
        trapFocus: true,
        props: props,
        events: {
          'submit': (new_data) => {
            if (sound.hotkey && new_data.hotkey !== sound.hotkey) {
              this.deregisterHotkey(sound.hotkey);
            }
            if (new_data.hotkey === '') {
              new_data.hotkey = null;
            }
            try {
              new_data.source = JSON.parse(new_data.source);
            } catch (e) {
              this.showWarning("failed to parse source for sound");
              return;
            }
            this.changeSound(soundboard_id, sound_id, new_data);
          },
          'delete': () => {
            this.deleteSound(soundboard_id, sound_id);
          }
        }
      });
    },
    changeSound: function (soundboard_id, sound_id, new_data) {
      let soundboard = this.soundboards.find((s) => s.id === soundboard_id);
      axios
        .put('/api/soundboards/' + soundboard.id + '/sounds/' + sound_id, {
          name: new_data.name,
          hotkey: new_data.hotkey,
          source: new_data.source,
        })
        .then((response) => {
          let sound = response.data.data;
          this.showSuccess(
            'changeSound: ' + sound.name + ' to ' + soundboard.name);

          Vue.set(soundboard.sounds, soundboard.sounds.findIndex((s) => s.id === sound.id), sound);

          if (sound.hotkey) {
            this.registerHotkey(sound.hotkey, {
              soundboard_id: soundboard_id,
              sound_id: sound.id,
            });
          }
        })
        .catch((error) => {
          this.showResponseError('changeSound', error);
          this.reloadData();
        });
    },
    addSound: function (soundboard_id, sound_id, name, source) {
      let soundboard = this.soundboards.find((s) => s.id === soundboard_id);
      axios
        .post(
          '/api/soundboards/' + soundboard.id + '/sounds/' + sound_id, {
          name: name,
          hotkey: null,
          source: source,
        },
          { headers: { 'x-method': 'create' } })
        .then((response) => {
          let sound = response.data.data;
          this.showSuccess(
            'addSound: ' + sound.name + ' to ' + soundboard.name);

          soundboard.sounds.splice(soundboard.sounds.findIndex((s) => s.id === sound_id) + 1, 0, sound);

        })
        .catch((error) => {
          this.showResponseError('addSound', error);
          this.reloadData();
        });
    },
    sendDeleteSound: function (soundboard_id, sound_id) {
      let soundboard = this.soundboards.find((s) => s.id === soundboard_id);
      let sound = soundboard.sounds.find((s) => s.id === sound_id);

      axios.delete('/api/soundboards/' + soundboard.id + '/sounds/' + sound.id)
        .then((response) => {
          this.showSuccess(
            'deleteSound: ' + sound.name + ' from ' + soundboard.name);

          soundboard.sounds.splice(soundboard.sounds.findIndex((s) => s.id === sound.id), 1);

        })
        .catch((error) => {
          this.showResponseError('deleteSound', error);
          this.reloadData();
        });
    },
    deleteSound: function (soundboard_id, sound_id, without_confirmation) {
      let soundboard = this.soundboards.find((s) => s.id === soundboard_id);
      let sound = soundboard.sounds.find((s) => s.id === sound_id);

      if (without_confirmation) {
        this.sendDeleteSound(soundboard_id, sound_id);
      } else {
        this.$buefy.dialog.confirm({
          message: 'Remove sound <b>' + sound.name +
            '</b> from soundboard <b>' + soundboard.name + '</b>?',
          type: 'is-danger',
          onConfirm: () => {
            this.sendDeleteSound(soundboard_id, sound_id);
          },
        });
      }
    },
    updateSoundboard: function (soundboard_id) {
      let soundboard = this.soundboards.find((s) => s.id === soundboard_id);

      axios
        .post('/api/soundboards/' + soundboard.id, {
          name: soundboard.name,
          hotkey: soundboard.hotkey,
          position: soundboard.position,
        })
        .then((response) => {
          this.showSuccess(
            'Updated soundboard name to: ' + response.data.data.name);
        })
        .catch((error) => {
          this.showResponseError('updateSoundboard', error);
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

  return url.protocol === 'http:' || url.protocol === 'https:';
}

window.addEventListener('touchmove', function () { });
