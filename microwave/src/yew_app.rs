use std::iter;

use tune::{midi::ChannelMessageType, temperament::EqualTemperament};
use wasm_bindgen::{prelude::*, JsValue};
use web_sys::{HtmlElement, KeyboardEvent, UrlSearchParams};
use yew::{html, Component, ComponentLink, Html, ShouldRender};

use crate::model::Model;

#[wasm_bindgen]
pub fn start_microwave() {
    yew::start_app::<AppModel>();
}

struct AppModel {
    link: ComponentLink<Self>,
    model: Result<(Model, EqualTemperament), String>,
}

enum Msg {
    StartNote(u8),
    StopNote(u8),
    KeyDown(KeyboardEvent),
    KeyUp(KeyboardEvent),
}

impl Component for AppModel {
    type Message = Msg;
    type Properties = ();

    fn create(_: Self::Properties, link: ComponentLink<Self>) -> Self {
        let window = web_sys::window().unwrap();
        let search = window.location().search().unwrap();
        let search_params = UrlSearchParams::new_with_str(&search).unwrap();
        let args_vec = search_params
            .get_all("arg")
            .iter()
            .filter_map(|js_value| js_value.as_string())
            .collect::<Vec<_>>();
        let args = iter::once("microwave".to_owned()).chain(args_vec);

        let model = crate::create_wasm_model_from_args(args);
        AppModel { link, model }
    }

    fn change(&mut self, _: Self::Properties) -> bool {
        false
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        if let Ok((model, _)) = &mut self.model {
            match msg {
                Msg::StartNote(key) => model
                    .engine
                    .handle_midi_event(ChannelMessageType::NoteOn { key, velocity: 100 }),
                Msg::StopNote(key) => model
                    .engine
                    .handle_midi_event(ChannelMessageType::NoteOff { key, velocity: 100 }),
                Msg::KeyDown(event) => {
                    handle_key_code_event(model, &event.code(), true);
                    handle_key_pressed_event(model, &event.key());
                }
                Msg::KeyUp(event) => {
                    handle_key_code_event(model, &event.code(), false);
                }
            }
            model.update();
        }
        true
    }

    fn view(&self) -> Html {
        match &self.model {
            Ok((model, temperament)) => {
                html! {
                    <div
                            onkeydown=self.link.callback(move |event| Msg::KeyDown(event))
                            onkeyup=self.link.callback(move |event| Msg::KeyUp(event))
                    >
                        <div>{ "Click on the buttons or use the keyboard to play notes. Use up/down to change the waveform. Arguments an be provided via query parameters of the form \"&arg=foo&arg=bar&...\". If you experience audio buffer underruns (crackling noise) increase the buffer size." }</div>
                        <div>
                            { format_args!("Waveform: {} - {}", model.waveform_number, model.waveforms[model.waveform_number].name()) }
                        </div>
                        <div>
                            { for (0..128).map(|index| html! {
                                <button id=format_args!("button-{}", index)
                                        style="color:red"
                                        onmousedown=self.link.callback(move |_| Msg::StartNote(index))
                                        onmouseup=self.link.callback(move |_| Msg::StopNote(index))
                                        onmouseleave=self.link.callback(move |_| Msg::StopNote(index))
                                >
                                    { format_args!("{}", temperament.get_heptatonic_name(i32::from(index) - 62)) }
                                </button>
                            })}
                        </div>
                    </div>
                }
            }
            Err(err) => {
                html! {
                    <textarea style="width:100%" rows=40 value=err readonly=true spellcheck=false/>
                }
            }
        }
    }

    fn rendered(&mut self, _first_render: bool) {
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let element = document.get_element_by_id("button-62").unwrap();
        let js_value: &JsValue = element.as_ref();
        HtmlElement::from(js_value.clone()).focus().unwrap();
    }
}

fn handle_key_pressed_event(model: &mut Model, key: &str) {
    match key {
        "Up" | "ArrowUp" => {
            model
                .engine
                .dec_program(&mut model.selected_program.program_number);
        }
        "Down" | "ArrowDown" => {
            model
                .engine
                .inc_program(&mut model.selected_program.program_number);
        }
        _ => {}
    }
}

fn handle_key_code_event(model: &mut Model, code: &str, pressed: bool) {
    let key_coord = match code {
        "Backquote" => (-6, 1),
        "Digit1" => (-5, 1),
        "Digit2" => (-4, 1),
        "Digit3" => (-3, 1),
        "Digit4" => (-2, 1),
        "Digit5" => (-1, 1),
        "Digit6" => (0, 1),
        "Digit7" => (1, 1),
        "Digit8" => (2, 1),
        "Digit9" => (3, 1),
        "Digit0" => (4, 1),
        "Minus" => (5, 1),
        "Equal" => (6, 1),
        "Backspace" => (7, 1),
        "Tab" => (-6, 0),
        "KeyQ" => (-5, 0),
        "KeyW" => (-4, 0),
        "KeyE" => (-3, 0),
        "KeyR" => (-2, 0),
        "KeyT" => (-1, 0),
        "KeyY" => (0, 0),
        "KeyU" => (1, 0),
        "KeyI" => (2, 0),
        "KeyO" => (3, 0),
        "KeyP" => (4, 0),
        "BracketLeft" => (5, 0),
        "BracketRight" => (6, 0),
        "Enter" => (7, 0),
        "CapsLock" => (-6, -1),
        "KeyA" => (-5, -1),
        "KeyS" => (-4, -1),
        "KeyD" => (-3, -1),
        "KeyF" => (-2, -1),
        "KeyG" => (-1, -1),
        "KeyH" => (0, -1),
        "KeyJ" => (1, -1),
        "KeyK" => (2, -1),
        "KeyL" => (3, -1),
        "Semicolon" => (4, -1),
        "Quote" => (5, -1),
        "Backslash" => (6, -1),
        "ShiftLeft" => (-7, -2),
        "IntlBackslash" => (-6, -2),
        "KeyZ" => (-5, -2),
        "KeyX" => (-4, -2),
        "KeyC" => (-3, -2),
        "KeyV" => (-2, -2),
        "KeyB" => (-1, -2),
        "KeyN" => (0, -2),
        "KeyM" => (1, -2),
        "Comma" => (2, -2),
        "Period" => (3, -2),
        "Slash" => (4, -2),
        "ShiftRight" => (5, -2),
        _ => return,
    };

    model.keyboard_event(key_coord, pressed);
}
