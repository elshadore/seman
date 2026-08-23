# SEMAN: SErvice MANager
SEMAN is a minimal linux service manager designed for the management of simple shell/binary programs often used with tiling window managers (such as the types listed below).

- (Menu) Bars
- Notification Managers
- Compositiors
- Keyboard Daemons.

SEMAN allows you to define, start, restart and kill these processes making minimal linux setups easier to work on. SEMAN also contains a primitive timer system for executing shell programs.

Here is an example of the shell interface.
```sh
seman server-start
seman defservice-start dunst dunst
seman timer foo 10min "echo \"henlo word!\""
seman timer-list
seman server-kill
```

The SEMAN server will bind to port `7676` by default, this can be configured by the environment variable `SEMAN_PORT`.

## SEMAN Commands
Below is a listing of all SEMAN commands.

- `server-start`
    Starts the SEMAN server as a *foreground* process.
    
- `server-kill`
    Kills the current SEMAN server.

- `server-status`
    Returns `server: ok!` if the server is active, returns `server: not found` if not server is found.

- `defservice`
    Define a simple service. Service names are unique, defining a service with the same name will overwrite and *kill* the previous service.
    - Argument 1: service name
    - Argument 2: service command

- `service-start`
    Start/restart a named service.
    - Argument 1: service name.

- `service-stop`
    Stops a service killing the process.

- `defservice-start`
    Combines `defservice` and `service-start`.
    - Argument 1: service name
    - Argument 2: service command

- `service-list` || `services`
    Provides a listing of all services, and wether they are active or not.
    - Argument Flag (optional): --json

- `timer`
    Creates a named timer. Timers can have the same name and will not override each other unlike services.
    - Argument 1: timer name
    - Argument 2: time
    - Argument 3 (optional): command ran on completion 

- `timer-list` || `timers`
    Returns a list of all currently active timers.
    - Argument Flag (optional): --json

- `timer-kill`
    Kills a timer(s) by name. If there are timers with the same name it kills them all.
    - Argument 1: timer name

- `exec`
    Executes a shell script. This will not block. So this will not return the scripts exitcode or stdout.

- `ping`
    Pong.

## Config File: `.semanrc`
When initially starting the `seman server-start`, the `.semanrc` config file will be ran. This is interpretted as a list of SEMAN commands. Server commands cannot be used this mode *obviously*. SEMAN will attempt to find this file in the following locations.

```sh
## SEMANRC env variable
$SEMANRC

## XDG HOME
${XDG_CONFIG_HOME}/.semanrc
${XDG_CONFIG_HOME}/seman/.semanrc

## FALLBACK HOME
${HOME}/.semanrc
${HOME}/seman/.semanrc

```

An example config.

```sh
defservice-start picom picom
defservice-start nm-applet nm-applet
defservice-start dunst dunst

timer _ 10min "echo \"henlo word!\""

exec some-script
```
