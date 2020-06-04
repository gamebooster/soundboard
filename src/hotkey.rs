// use ::hotkey as hotkeyExt;

// pub struct Hotkeys {
//     listener: hotkeyExt::Listener
// }

// impl Hotkeys {
//     pub fn new() {
//         let hotkey_thread = std::thread::spawn(move || {
//             let mut hk = hotkeyExt::Listener::new();
//             hk.listen();
//         });
//     }
//     fn register(&self) -> Result<()> {
//         hk.register_hotkey(
//             sound
//                 .hotkey_modifier
//                 .iter()
//                 .fold(0, |acc, x| acc | (*x as u32)) as u32,
//             sound.hotkey_key as u32,
//             move || {
//               let tx_clone = tx_clone.clone();
//               send_playsound(tx_clone, Path::new(&sound.path));
//             },
//         )
//         .or_else(|_s| Err(anyhow!("register key")));
//         for sound in config_file.sounds.unwrap_or(Vec::new()) {
//             let tx_clone = tx_clone.clone();
//             let _result = hk
//                 .register_hotkey(
//                     sound
//                         .hotkey_modifier
//                         .iter()
//                         .fold(0, |acc, x| acc | (*x as u32)) as u32,
//                     sound.hotkey_key as u32,
//                     move || {
//                       let tx_clone = tx_clone.clone();
//                       send_playsound(tx_clone, Path::new(&sound.path));
//                     },
//                 )
//                 .or_else(|_s| Err(anyhow!("register key")));
//         }
//     }
// }