# ToDo

## Stream April 23th TODO

- [ ] Create a Conversation Model that front and backend can use
  - I want to be able to do more than a one-off question answer
  - The server needs to be able to respond with multiple messages for a given question
  - This should allow polling on the frontend
  - Maybe this is a Sqlite DB, that we keep in mem for now, but could persist later
- [ ] Add 'model' layer to be able to test different strategies
  - Current model is one. ReAct model will be seperate
- [ ] Make UI look a bit more chat like

## Stream April 16th TODO

- [x] Be able to enter a search and find relevant sentences via embeddings
- [x] Feed question and context into ChatGPT for Answer
- [x] Add Yew Frontend
- [ ] Add embeddings for full posts
  - When we started I decided to do embeddings at the sentence level. But from reading more about LLM and embeddings
  it seems like I can/probably want to use bigger chunks
