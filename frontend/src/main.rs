use gloo_net::http::Request;
use shared::{ChatRequest, AnswerResp};
use uuid::Uuid;
use yew::prelude::*;



const APP_URL: Option<&str> = option_env!("APP_URL");

#[function_component]
fn App() -> Html {
    let app_url = APP_URL.unwrap_or("http://localhost:3000");

    let chat_api_url = format!("{app_url}/api/v0/chat");
    let textarea_ref = use_node_ref();

    let question: UseStateHandle<Option<String>> = use_state(|| None);

    let answer: UseStateHandle<Option<String>> = use_state(|| None);
    let prompt: UseStateHandle<Option<String>> = use_state(|| None);

    let conversation_slug: UseStateHandle<Uuid> = use_state(Uuid::new_v4);

    let onsubmit = {
        let question = question.clone();
        let answer = answer.clone();
        let prompt = prompt.clone();
        let textarea_ref = textarea_ref.clone();

        move |e: SubmitEvent| {
            let textarea = textarea_ref.cast::<web_sys::HtmlInputElement>().unwrap();
            let value = textarea.value();

            question.set(Some(value));
            answer.set(None);
            prompt.set(None);

            e.prevent_default();
        }
    };

    {
        let question = question.clone();
        let answer = answer.clone();
        let prompt = prompt.clone();
        let conversation_slug = conversation_slug.clone();

        use_effect_with_deps(
            move |question| {
                let question = question.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    let Some(q) = question.as_ref() else {
                        return 
                    };
                    let req = ChatRequest { question: q.to_owned(), conversation_slug: *conversation_slug };

                    let answer_resp: AnswerResp =
                        Request::post(&chat_api_url)
                            .json(&req).unwrap()
                            // .body(q.to_owned())
                            .send()
                            .await
                            .unwrap()
                            .json()
                            .await
                            .unwrap();

                    answer.set(Some(answer_resp.answer));
                    prompt.set(Some(answer_resp.context));
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
                        if let Some(p) = prompt.as_ref() {
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
