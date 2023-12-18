# chess-rs
Online chess server that allows to play 1v1 matches utilizing long algebraic notation in CLI.

# Commands
- `/help`
- `/log in %username%`
- `/play`
- `/concede`
- `/statistics`, `/stats`⏳🙄
- `:` - chat message
- `e2e4` - chess move in long algebraic notation

# Features
1. Chess! 
2. Chat
3. User game history⏳🙄
4. Web admin panel⏳🙄
5. Metrics ⏳🙄

# Implementation
1. Async using `Tokio`
2. Logging using `log` and `env_logger`
3. Serialization using `Serde`
4. Chess using `chess`
5. Errors with `thiserror` 
6. Database - `Postgre SQL` ⏳🙄
7. Web frontend ⏳🙄
8. Metrics using `Prometheus` ⏳🙄