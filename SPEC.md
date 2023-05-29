# Silly Goals

Goal tracking application, but with attitude to be less boring and pressury.


## Variants

### Gentle

- Kind greeting
- stages
  - had an idea
  - getting going
  - almost there
  - yayyyyyyy
- no deadlines
- silently hide old unmet goals (archive page)


### Business (silly)

- Businessy greeting
- stages
  - brainstorming
  - telling your boss it's almost done
  - it's actually almost done
  - eh good enough
- soft deadlines (no difference, just language)
- formal question to deal with unmet goals

### Serious

- Formal greeting
- stages
  - in queue
  - in progress
  - finishing touches
  - completed
- hard deadlines
- formal question to deal with unmet goals

### Mean

- Hostile greeting
- stages
  - get to work lazy
  - hey you actually did something
  - you're not done yet?
  - oh finally, jeez
- very mean about deadlines (bold, red, green, etc.)
- nag about unmet goals angrily

### Just colors

- colorful greeting?
- stages
  - red
  - yellow
  - blue
  - green
- no deadlines
- new color for umet goals

### Boring

- [insert greeting]
- stages
  - stage 1
  - stage 2
  - stage 3
  - stage 4
- gray deadlines
- [generic]

### Custom?
- greeting
- stages
- deadlines
- unmet goal behavior


## Features

- [x] Setting of sassiness mode, and per-group
- [x] Categories of goals
- [x] User auth with webauthn
- [x] drag and drop kanban
- [x] optional deadlines
- [x] CRUD
  - [x] update changing stage of thing (number)
  - [x] update name of thing

## Tables

- Users
  - ids
  - unique nanoid (index)
  - email address
- Credentials (check polls project for sql)
  - id
  - credential
  - user_id
- Group
  - name
  - description
  - user id
  - association to goals (preload)
  - sassiness id
   icon?
- Goals
  - title
  - description
  - stage
  - category id (index)
  - optional deadline
- Tone (auto create builtins, maybe memoize somehow?)
  - name
  - user id (index, optional)
  - global (bool) 
  - stages [num => word, color]
  - greeting
  - unmet goal behavior
  - deadline options



## Other stuff

- [ ] good timezone support

TODO: possibly implement old goals behavior
TODO: fix missing deadlines on changed tone
TODO: easter egg about the silly ghoul?
TODO: improve accessibility for modals
TODO: properly htmx-ify new group form or don't
TODO: handle logged out users making htmx requests better
TODO: setup outgoing mail from sillygoals domain
TODO: make get started button larger
TODO: registration email error handler
TODO: handle soft deadlines and hard deadlines
TODO: fix layout on mobile, mobile swap after tutorial lifts footer onto screen
