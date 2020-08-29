use wasm_bindgen::prelude::*;

use crate::CliError;
use std::iter;
use yew::{html, Component, ComponentLink, Html, InputData, ShouldRender};

#[wasm_bindgen]
pub fn start_tune_cli() {
    yew::start_app::<Model>();
}

pub struct Model {
    link: ComponentLink<Self>,
    args: String,
    stdin: String,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

pub enum Msg {
    ArgsInput(String),
    StdinInput(String),
    RunTuneCli,
    CopyToStdin,
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_: Self::Properties, link: ComponentLink<Self>) -> Self {
        Model {
            link,
            args: "scl\nsteps\n1:31:2".to_owned(),
            stdin: String::new(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    }

    fn change(&mut self, _: Self::Properties) -> bool {
        false
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::ArgsInput(args) => self.args = args,
            Msg::StdinInput(stdin) => self.stdin = stdin,
            Msg::RunTuneCli => {
                let args = iter::once("tune")
                    .chain(self.args.lines())
                    .map(str::trim)
                    .map(str::to_owned);
                self.stdout.clear();
                self.stderr.clear();
                let result = crate::run_in_wasm_env(
                    args,
                    self.stdin.as_bytes(),
                    &mut self.stdout,
                    &mut self.stderr,
                );

                match result {
                    Ok(()) => {}
                    Err(CliError::CommandError(err)) => self.stderr.extend(err.as_bytes()),
                    Err(CliError::IoError(err)) => self.stderr.extend(err.to_string().as_bytes()),
                };
            }
            Msg::CopyToStdin => self.stdin = String::from_utf8_lossy(&self.stdout).into_owned(),
        }
        true
    }

    fn view(&self) -> Html {
        html! {
            <div>
                <div style="width:50%; float:left" >
                    <label for="args-area">{"command line arguments"}</ label>
                    <textarea id="args-area"
                            style="width:100%; resize:none"
                            rows=10
                            spellcheck=false
                            value=self.args
                            placeholder="Type one command line argument per row"
                            oninput=self.link.callback(|e: InputData| Msg::ArgsInput(e.value))
                    />
                </div>
                <div style="width:50%; float:left" >
                    <label for="stdin-area">{"stdin"}</ label>
                    <textarea id="stdin-area"
                            style="width:100%; resize:none"
                            rows=10
                            spellcheck=false
                            value=self.stdin
                            placeholder="Paste STDIN here"
                            oninput=self.link.callback(|e: InputData| Msg::StdinInput(e.value))
                    />
                </div>
                <div>
                    <button style="color:red" onclick=self.link.callback(|_| Msg::RunTuneCli)>{ "Run tune-cli" }</button>
                </div>
                <div style="width:50%; float:left" >
                    <label for="stdout-area">{"stdout"}</label>
                    <textarea id="stdout-area"
                            style="width:100%; resize:none"
                            rows=30
                            readonly=true
                            value=String::from_utf8_lossy(&self.stdout)
                    />
                </div>
                <div style="width:50%; float:left">
                    <label for="stderr-area">{"stderr"}</label>
                    <textarea id="stderr-area"
                            style="width:100%; resize:none"
                            rows=30
                            readonly=true
                            value=String::from_utf8_lossy(&self.stderr)
                    />
                </div>
                <div>
                    <button style="color:blue" onclick=self.link.callback(|_| Msg::CopyToStdin)>{ "Copy to stdin" }</button>
                </div>
            </div>
        }
    }
}
