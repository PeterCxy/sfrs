SFRS
---

Main repository: <https://cgit.typeblog.net/sfrs/>. __The GitHub repository is merely a mirror.__

SFRS is an implementation of the synchronization server of Standard Notes, written in Rust. It is intended for personal usage, especially for those that would like to self-host the server for maximum possible privacy, even when Standard Notes already uses purely client-side encryption by default.

Standard Notes is a free and open-source note-taking application that focuses on simplicity and longevity. Please refer to [their website](https://standardnotes.org/) for further information on Standard Notes itself.

Contact
---

For issues and patches, please use the following contact methods

- Google Groups: petercxy-projects@googlegroups.com ([web](https://groups.google.com/forum/?oldui=1#!forum/petercxy-projects))
- Matrix Chat Room: #petercxy-projects:neo.angry.im

Disclaimer
---

This project was created for personal purposes, as a hobby, and has never gone through any security audit. It is not yet production-ready. Though I have been dog-fooding this project for some time, there is ABSOLUTELY NO GUARANTEE that this software will work as intended for any period of time. The project may or may not introduce breaking changes that could result in the need to re-initialize the database and re-import all your notes from your local backups. It is your responsibility to keep regular backups for your personal data.

Installation (Non-Docker)
---

There is no binary releases provided currently. You need to have Rust Nightly installed in your system to build this project. You can consult <https://rustup.rs> to learn how to install the nightly version of Rust in your OS.

After Rust Nightly is installed, you can simply run

```bash
cargo build --release
```

in the directory of this project to build the binary. The binary will be located in `target/release/sfrs`.

Installation (Docker)
---

I have provided a `Dockerfile` in `docker/Dockerfile`. You can run the following command

```bash
docker build -f docker/Dockerfile .
```

to build a Docker container for this project. The Docker container by default uses a Docker volume located at `/data` inside the container for storing configuration and database. You will need to mount a volume at this location for your data to persist. The internal port provided by the container is `8000`.

Configuration
---

SFRS reads configuration from a file named `.env` in the current working directory (this is set to `/data` by default in the Docker build). The file should contain content like the following:

```bash
SFRS_ENV=production
DATABASE_URL=<filename_of_your_database>
SYNC_TOKEN_SECRET=<generate_something_random_at_first_configuration>
SYNC_TOKEN_SALT=<generate_something_random_at_first_configuration>
```

Replace everything in `<>` (inclusive) with the instructions inside those brackets. The database will be created at first start.

By default, the program listens at `127.0.0.1:8000`, though the port can be changed by setting the variable `ROCKET_PORT` in either `.env` or in environment variables.

It is necessary to place a reverse-proxy in front of SFRS. The reverse-proxy should be configured with a trusted SSL certificate. To allow the import function of the client to work properly, you need to set the max acceptable body size (in Nginx it's called `client_max_body_size`) to something bigger than the default value, e.g. `10M` or `50M`.

Caveats
---

* Backups through the CloudLink plugin is not supported. I may or may not introduce similar backup features in the future, but it probably won't be through the plugin, because it's outside the standard and could become incompatible at any time. I suggest you for now use the automatic local backup feature of the Desktop clients of Standard Notes to keep backups.
* There are tests written, but no tests for the synchronization feature is currently implemented. However, I have taken some precautions while writing the synchronization algorithm, including a per-user mutex lock and a different `sync_token` that is not a timestamp. There are plentiful of comments in the source code of the sync interface (which I think is better than the official implementation), so please check if you are not sure.
* SFRS uses SQLite as the database engine, as it was never designed for large-scale usage. Its focus is on security, simple configuration and maintanence. If you are looking for something that can support thousands of users synchronizing at the same time, SFRS is probably not for you.
* SFRS is licensed under AGPLv3, which may be a bummer for some people, though I probably won't have time or money to spare to sue anybody for anything.
