# FluxDM - Rust Download Manager

> **A native, high-performance download manager built with Rust**  
> Windows-first, Linux-ready. Evolving through three deliberate phases: IDM → JDownloader → P2P Hybrid.

[![Status](https://img.shields.io/badge/status-pre--development-yellow)](docs/project/DEVELOPMENT_CHECKLIST.md)
[![Phase](https://img.shields.io/badge/phase-0%3A%20education-blue)](docs/learning/education-guide.md)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange)](https://www.rust-lang.org/)

---

## Project Vision

FluxDM is a **learning-first project** aimed at building a production-quality download manager while mastering Rust and modern software engineering practices. This is **not a race to production**—it's a carefully crafted learning journey.

**Current Phase**: Pre-Development Education (2-4 weeks)  
**First Milestone**: Phase 1 - IDM-level functionality

---

## Documentation

**Start Here**: [`docs/README.md`](docs/README.md) - Documentation index

### Quick Links

- **[Education Guide](docs/learning/education-guide.md)** - 10-task learning path (START HERE)
- **[Development Checklist](docs/project/DEVELOPMENT_CHECKLIST.md)** - Task tracking
- **[Architecture Overview](docs/architecture/overview.md)** - System design
- **[Rust Style Guide](docs/engineering/rust-style-guide.md)** - Coding standards
- **[UI Guidelines](docs/design/ui-guidelines.md)** - Design principles

### Full Specification

- **[Master Design Document](docs/rust_download_manager_design.md)** - Complete project spec

---

## Three-Phase Development Strategy

### Phase 1: IDM-Level Foundation (Current Focus)

**Goal**: Build a stable, fast download manager that can replace IDM in daily use

**Features**:
- Multi-part HTTP downloads with resume
- Browser extension integration (Chrome/Firefox/Edge)
- Real-time UI with WebSocket updates
- Speed throttling and scheduling
- System tray integration

**Success Criteria**: Can replace IDM for 2 weeks without frustration

### Phase 2: JDownloader-Style Automation

**Only after Phase 1 is production-ready**

- LinkGrabber system (clipboard monitoring, link extraction)
- Plugin architecture (host plugins, decrypters)
- Automation engine (rules, scheduling)

### Phase 3: P2P Hybrid System

**Only after Phase 2 is production-ready**

- Lightweight DHT peer discovery
- Hybrid HTTP + P2P chunk sources
- VPN binding & network isolation

---

## Tech Stack

**Core**:
- **Rust** - Systems programming language
- **Tokio** - Async runtime
- **Reqwest** - HTTP client with connection pooling
- **sqlx** - Type-safe database queries
- **Axum** - Web framework for API

**UI** (Decision pending - Task 8):
- **Slint** (recommended) - Native, lightweight binaries
- **Tauri** (alternative) - Web-based UI

**Browser**:
- **WebExtensions** - Cross-browser extension API

See [`docs/architecture/library-choices.md`](docs/architecture/library-choices.md) for detailed rationale.

---

## Getting Started

### For Learners (Recommended Path)

**You're in the education phase.** Before writing any production code:

1. **Read**: [`docs/learning/education-guide.md`](docs/learning/education-guide.md)
2. **Complete**: 10 learning tasks (2-4 weeks)
3. **Verify**: Answer all verification questions
4. **Document**: Your learnings in `docs/learning/`

**Important**: Don't skip to Phase 1 development until education is complete.

### For Developers (Post-Education)

```powershell
# Clone repository
git clone https://github.com/Fluxaus/FluxDM.git
cd FluxDM

# Build (after workspace is initialized)
cargo build --release

# Run tests
cargo test --workspace

# Run application (after Phase 1 complete)
cargo run
```

---

## Core Tenets

1. **Learning Before Shipping** - Deep understanding >>> rushing to features
2. **Phased Development** - Complete Phase 1 before Phase 2, then Phase 3
3. **Windows-First, Linux-Ready** - Design for portability from day one
4. **Lightweight Native Binaries** - Target <15MB, avoid Electron bloat
5. **Zero Panics Policy** - Always use `Result`/`?`, never `unwrap()` in production
6. **Event-Driven Architecture** - State changes via broadcast channels, not polling
7. **Documentation as Code** - Document decisions, learnings, and tradeoffs
8. **Test-Driven Reliability** - Write tests alongside code from Day 1

---

## Project Status

**Current Phase**: Phase 0 (Education)  
**Version**: 0.0.0  
**Last Updated**: December 2, 2025

### Education Progress (10 Tasks)

- [ ] Task 1: Rust Async & Tokio Fundamentals
- [ ] Task 2: Error Handling Patterns
- [ ] Task 3: HTTP Downloads & Range Requests
- [ ] Task 4: SQLite & sqlx Patterns
- [ ] Task 5: WebSocket Real-Time Communication
- [ ] Task 6: Browser Extension Architecture
- [ ] Task 7: Rust Module System & Workspaces
- [ ] Task 8: UI Framework Decision (Slint vs Tauri)
- [ ] Task 9: Testing Strategy
- [ ] Task 10: Semantic Versioning & Releases

See [`docs/project/DEVELOPMENT_CHECKLIST.md`](docs/project/DEVELOPMENT_CHECKLIST.md) for detailed progress.

---

## Contributing

This is currently a **personal learning project**. Contributions are welcome after Phase 1 is complete and stable.

If you'd like to follow along or learn from this project:
1. Star the repository
2. Read the documentation in `docs/`
3. Open issues for questions or suggestions

---

## License

[License TBD - will be added before public release]

---

## Acknowledgments

**Inspiration**:
- **Internet Download Manager (IDM)** - Simplicity and speed
- **JDownloader** - Automation and extensibility
- **aria2** - P2P hybrid downloads

**Learning Resources**:
- [The Rust Book](https://doc.rust-lang.org/book/)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [Async Rust Book](https://rust-lang.github.io/async-book/)

---

**Note**: This is a learning-first project. Quality of understanding over speed of development.
