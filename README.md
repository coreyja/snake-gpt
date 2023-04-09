# snake-gpt

Chatbot for Battlesnake

## Thoughts

We want to build a ChatBot that can answer arbitrary questions related to Battlesnake.
We want it to be able to answer questions about different strategies, the rules and also
about the current results and top snakes.

## Architecture

### Semantic Search

First we will take the users query and perform a semantic search based on it and find potentially relevant
content from the Battlesnake Docs (plus other sources).

We will use the OpenAI Embeddings API for power the text -> vector transformation.
Then we will use sqlite and [`sqlite-vss`]( https://github.com/asg017/sqlite-vss )

### ChatGPT

We will make a prompt using the above information as context, with the users full query included. ChatGPT will
be responsible for taking the contexts and question and forming a result

#### Ideas

##### Sentence Splitting

I wonder if we could feed the Markdown into ChatGPT and ask it to make each sentence on its own line.
Then we could simply use `lines()` to split it up.
