use std::sync::Arc;

use gloo_net::http::Request;
use shared::{playground::Api, ChatRequest, ConversationResponse};
use uuid::Uuid;
use yew::prelude::*;
use yew_hooks::use_interval;

const APP_URL: Option<&str> = option_env!("APP_URL");

mod rpc;

#[function_component]
fn App() -> Html {
    let app_url = APP_URL.unwrap_or("http://localhost:3000").to_string();

    let client = rpc::Client::new(Some(app_url));
    let client = Arc::new(client);

    let textarea_ref = use_node_ref();

    let question: UseStateHandle<Option<String>> = use_state(|| None);

    let answer: UseStateHandle<Option<String>> = use_state(|| None);
    let context: UseStateHandle<Option<String>> = use_state(|| None);

    let conversation_slug: UseStateHandle<Uuid> = use_state(Uuid::new_v4);

    let onsubmit = {
        let question = question.clone();
        let answer = answer.clone();
        let context = context.clone();
        let textarea_ref = textarea_ref.clone();

        move |e: SubmitEvent| {
            let textarea = textarea_ref.cast::<web_sys::HtmlInputElement>().unwrap();
            let value = textarea.value();

            question.set(Some(value));
            answer.set(None);
            context.set(None);

            e.prevent_default();
        }
    };

    {
        let answer = answer.clone();
        let context = context.clone();
        let question = question.clone();
        let conversation_slug = conversation_slug.clone();
        let client = client.clone();
        use_interval(
            move || {
                let conversation_slug = conversation_slug.clone();
                let answer = answer.clone();
                let context = context.clone();
                let question = question.clone();
                let client = client.clone();

                if question.is_none() || answer.is_some() {
                    return;
                }

                wasm_bindgen_futures::spawn_local(async move {
                    let answer_resp = client
                        .get_conversation(conversation_slug.to_string())
                        .await
                        .unwrap();

                    answer.set(answer_resp.answer);
                    context.set(answer_resp.context);
                });
            },
            1000,
        );
    }

    {
        let question = question.clone();
        let answer = answer.clone();
        let context = context.clone();
        let conversation_slug = conversation_slug.clone();
        let client = client.clone();

        use_effect_with_deps(
            move |question| {
                let question = question.clone();
                let context = context.clone();
                let client = client.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    let Some(q) = question.as_ref() else {
                        return
                    };
                    let req = ChatRequest {
                        question: q.to_owned(),
                        conversation_slug: *conversation_slug,
                    };

                    let answer_resp = client.start_chat(req).await.unwrap();

                    answer.set(answer_resp.answer);
                    context.set(answer_resp.context);
                });
            },
            question,
        );
    }

    html! {
        <div>
            <div class="break-words">{"Conversation: "}{ *conversation_slug }</div>
            <form onsubmit={onsubmit.clone()}>
                <div class="flex flex-cols w-100vw">
                    <textarea
                        ref={textarea_ref}
                        placeholder="Enter your Battlesnake Question" rows=10 cols=50
                        class="w-1/2 shrink-0"
                        onkeydown={move |e: KeyboardEvent| {
                            if e.key() == "Enter" {
                                e.prevent_default();
                                onsubmit(SubmitEvent::new("").unwrap());
                            }
                            }}
                    />
                    <div class="shrink overflow-scroll max-h-[50vh]">
                        if let Some(p) = context.as_ref() {
                            <pre class="break-words">{ p }</pre>
                        }
                    </div>
                </div>
                <button>{ "Submit" }</button>
            </form>
            if let Some(q) = question.as_ref() {
                <p>{"Question: "}{ q }</p>
            }
            if let Some(a) = answer.as_ref() {
                <p>{"Answer: "}{ a }</p>
            }
        </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
