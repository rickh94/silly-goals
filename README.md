# Silly Goals

A super-fast SaaS site built with rust, actix-web, and htmx, with a
fully-passwordless authentication system.

[Check out the site](https://sillygoals.com)

or

[Watch the Video](https://sillygoals.com/about/video)

## About

### Track Your Goals

Keep track of what you want/need to to, but with an appropriate tone. Have a little fun! Things don't have to be serious all the time, get a little silly.

Watch our video to see what it's like to use!

#### Motivation style

We all have different ways of being motivated. Maybe you need nice gentle reminders without too much pressure. Or snarky and mean keeps you going. Or just basic and simple. Choose one of our pre-defined tones or create your own.

#### Goal Groups

Divide your goals into groups and set different tones for different groups. Or keep them all in one group.

#### Custom Deadlines

Sometimes it's important to have hard deadlines. Sometimes it's important to not have deadlines. Our pre-defined tones handle deadlines in different ways, or define your own with soft, hard, or no deadlines.

#### Kanban

Ok so it's basically a Kanban app, but it's sillier!


## Tech

### Rust

We all love rust. Type safe and if it compiles it works. Just a joy

### Actix Web

[Actix Web](https://actix.rs/) is probably the largest web framework for Rust and
it's the one that I know. It worked quite well for building up this application.


### Sqlite + Litestream + sqlx

Using sqlite, and [Litestream](https://litestream.io), the database can be 
right on the web server, super fast, and still have the durability and rollback 
of a managed database service. It's also very simple to use and deploy
compared to a full postgres setup. 

[Sqlx](https://github.com/launchbadge/sqlx) provides compile check sql queries,
just incredible for correctness and developer experience. It can also help with
fast iteration, because rather than looking up the correct sqlx, you can just 
guess and get an error code right in your editor.

### Passwordless Auth and Passkeys

The passwordless authentication has two core components: email and passkeys.
Email auth is straightforward, a temporary code is generated stored on the
session, and then sent to the user's email. (I'm using redis for session storage,
obviously this would be horribly insecure using any kind of client-side session)
Pretty basic stuff at this point.

Passkeys (WebAuthn, FIDO2, so many names…) use device based authenticators and
public key auth to verify users. It's more secure and much more user friendly.
I'm using the [webauthn-rs](https://github.com/kanidm/webauthn-rs) crate 
and [Simple WebAuthn](https://simplewebauthn.dev) javascript library to do 
most of the heavy lifting (and the nightmare that is base64 encoding and 
decoding). Logged in users can register their device and then login with just
that device next time.


### HTMX

[HTMX](https://htmx.org) gets me very close to my goal of never writing any 
javascript, or, at least, not needing a javascript framework. I wrote the whole
thing as a traditional Get-Post-Redirect server-side web app, and then enhanced
everything with htmx with just a few lines of rust (and a custom extractor).
With htmx's dynamic swapping of components, it's simple to send back chunks of
html, rendered from the same templates, and gives the user a mostly reload-free
experience that feels as responsive as a SPA. Additionally, if a user does 
reload, they will find themselves just where they were before, because the same
endpoint will render the whole page if the request isn't made from htmx. 

As a result of using htmx, the total download size of the site is quite small 
compared to a SPA, and using htmx often allows me to skip database quries, which
is huge in io-bound applications like websites.



### Tailwind

Yeah it's tailwind. It's the best css framework. I don't want to write my own 
css, I want to make things that look nice. If I need to change something, I'll
just use [sed](https://www.gnu.org/software/sed/manual/sed.html).



## Deployment

### Almost good enough

I tried a bunch of stuff that seems super cool [shuttle.rs](https://shuttle.rs)
seems awesome, but scale-to-zero means sometimes the site just isn't there, and
[fly.io](https://fly.io) is cool but free-ish and just not all that special.

### How I did it

It's on a server. Turns our, for small sites, docker-compose on a free-tier OCI 
server is just fine. I don't like forgetting what's all on a server though, so
I used [NixOs](https://www.nixos.org/) and 
[Arion](https://docs.hercules-ci.com/arion/) and deployed it using 
[morph](https://github.com/DBCDK/morph). This means it's all declarative, so 
in six months I can just read the config file and I'll know exactly what's 
running and can just move it to another server. Since litestream auto-downloads
the latest database, it would just work™. Not that I'm going to on purpose.
