# chess-rs
Online chess server that allows to play 1v1 matches utilizing long algebraic notation in CLI.

# Commands
- `/help`
- `/log in %username%`
- `/play`
- `/concede`
- `/statistics`, `/stats`
- `:` - chat message
- `e2e4` - chess move in long algebraic notation

# Features
1. Chess! 
2. Chat
3. User game history
4. Web admin panel
5. Metrics 

# Implementation
1. Async using `Tokio`
2. Serialization using `Serde`
3. Errors with `anyhow` and `thiserror` 
4. Database - `Postgre SQL` (?)
5. Web frontend with ...
6. Metrics using `Prometheus`