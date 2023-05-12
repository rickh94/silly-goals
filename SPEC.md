# Sassy Goals

Goal tracking application, but with attitude to be less boring and pressury.


## Variants

### Gentle

- Kind greeting
- stages
  - had an idea
  - thinkin about it
  - getting going
  - almost there
  - yayyyyyyy
- no deadlines
- silently hide old unmet goals (archive page)


### Business (silly)

- Businessy greeting
- stages
  - brainstorming
  - synergizing
  - telling your boss it's almost done
  - it's actually almost done
  - eh good enough
- soft deadlines (no difference, just language)
- formal question to deal with unmet goals

### Serious

- Formal greeting
- stages
  - in queue
  - started
  - in progress
  - finishing touches
  - completed
- hard deadlines
- formal question to deal with unmet goals

### Mean

- Hostile greeting
- stages
  - you haven't started yet
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
  - lime
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
  - stage 5
- gray deadlines
- [generic]

### Custom?
- greeting
- stages
- deadlines
- unmet goal behavior


## Features

- [ ] Setting of sassiness mode, global default and per-category
- [ ] Categories of goals
- [ ] User auth with webauthn
- [ ] Light and Dark mode
- [ ] Font options
- [ ] optional deadlines
- [ ] CRUD
  - [ ] update changing stage of thing (number)
  - [ ] update name of thing

## Tables

- Users
  - ids
  - unique nanoid (index)
  - email address
  - credential ids
- Credentials (check polls project for sql)
  - id
  - credential
- Categories
  - name
  - description
  - user id
  - association to goals (preload)
  - sassiness id
- Goals
  - name
  - description
  - stage
  - category id (index)
  - optional deadline
- Sassiness (auto create builtins, maybe memoize somehow?)
  - name
  - user id (index, optional)
  - global (bool) 
  - stages [num => word, color]
  - greeting
  - unmet goal behavior
  - deadline options



## Other stuff

- [ ] good timezone support
