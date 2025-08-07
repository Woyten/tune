#![allow(clippy::redundant_closure)]
#![recursion_limit = "512"]

use std::iter;

use material_yew::MatButton;
use material_yew::MatTextArea;
use yew::prelude::*;

pub fn main() {
    yew::Renderer::<Model>::new().render();
}

pub struct Model {
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
    PreventTextAreaEdit,
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Model {
            args: "scl\nsteps\n1:31:2".to_owned(),
            stdin: String::new(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::ArgsInput(args) => self.args = args,
            Msg::StdinInput(stdin) => self.stdin = stdin,
            Msg::RunTuneCli => {
                self.stdout.clear();
                self.stderr.clear();

                let args = iter::once("tune")
                    .chain(self.args.lines())
                    .map(str::trim)
                    .map(str::to_owned);

                tune_cli::run_in_wasm_env(
                    args,
                    self.stdin.as_bytes(),
                    &mut self.stdout,
                    &mut self.stderr,
                );
            }
            Msg::CopyToStdin => self.stdin = String::from_utf8_lossy(&self.stdout).into_owned(),
            Msg::PreventTextAreaEdit => {}
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <div style="height: 100%; display:grid; grid-template-columns: 1fr 1fr; grid-template-rows: min-content min-content auto; gap: 16px; padding: 8px; box-sizing: border-box">
                <div style="grid-row: 1; grid-column: 1">
                    <MatTextArea label="Command line arguments (one per row)"
                        rows=10
                        value={self.args.clone()}
                        oninput={ctx.link().callback(|text| Msg::ArgsInput(text))}
                    />
                </div>
                <div style="grid-row: 2; grid-column: 1">
                    <span style="--mdc-theme-primary: red" onclick={ctx.link().callback(|_| Msg::RunTuneCli)}>
                        <MatButton label="Run tune-cli" raised={true} />
                    </span>
                </div>
                <div style="grid-row: 3; grid-column: 1">
                    <MatTextArea label="STDOUT"
                        value={String::from_utf8_lossy(&self.stdout).into_owned()}
                        oninput={ctx.link().callback(|_| Msg::PreventTextAreaEdit)}
                    />
                </div>
                <div style="grid-row: 1; grid-column: 2">
                    <MatTextArea label="Paste STDIN here"
                        rows=10
                        value={self.stdin.clone()}
                        oninput={ctx.link().callback(|text| Msg::StdinInput(text))}
                    />
                </div>
                <div style="grid-row: 2; grid-column: 2">
                    <span style="--mdc-theme-primary: blue" onclick={ctx.link().callback(|_| Msg::CopyToStdin)} >
                        <MatButton label="Copy STDOUT to STDIN" raised={true} />
                    </span>
                </div>
                <div style="grid-row: 3; grid-column: 2">
                    <MatTextArea label="STDERR"
                        value={String::from_utf8_lossy(&self.stderr).into_owned()}
                        oninput={ctx.link().callback(|_| Msg::PreventTextAreaEdit)}
                    />
                </div>
            </div>
        }
    }
}
