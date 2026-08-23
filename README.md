# SEMAN: SErvice MANager
SEMAN is a simple linux service manager designed for usage with simple (tiling) window managers. It is designed for the management of simple desktop shell/binary programs such as listed below.

- Notification Managers
- Compositiors
- Keyboard Daemons.

SEMAN allows you to define, start, restart and kill these processes to make simple linux desktops easier to work on. SEMAN also contains a simple timer system for executing shell programs, this can also be used to linux desktop related stuff as well.

Here is an example of the shell interface.
```sh
seman timer foo 10min "echo \"henlo word!\""
seman timer-list
```

## SEMAN Commands
Below is a listing of all SEMAN commands.

### `server-start`
Starts the SEMAN server. *WARNING* like all linux socket/TCP stuff you can kill the server and start it only to find the only process is still bound to the port.

### `server-kill`
Kills the current SEMAN server. The same warning as in `server-start` also applies here.

### `server-status`
Returns `server: ok!` if the server is active, returns `server: not found` if not server is found.

### `defservice`
Define a simple service. Service names are unique, defining a service with the same name will overwrite the previous service.
- Argument 1: service name
- Argument 2: service command

### `service-start`
Start/restart a named service.
- Argument 1: service name.

### `service-stop`
Stops a service killing the process.

### `defservice-start`
Combines `defservice` and `service-start`.
- Argument 1: service name
- Argument 2: service command

### `service-list` || `services`
- Argument Flag (optional): --json
Provides a listing of all services, and wether they are active or not.

### `timer`
Creates a named timer. Timers can have the same name and will not override each other unlike services.
- Argument 1: timer name
- Argument 2: time
- Argument 3 (optional): command ran on completion 

### `timer-list` || `timers`
- Argument Flag (optional): --json
Returns a list of all currently active timers.

### `timer-kill`
Kills a timer(s) by name. If there are timers with the same name it kills them all.
- Argument 1: timer name

### `exec`
Executes a shell script. This will not block. So this will not return the scripts exitcode or stdout.

### `ping`
Pong.

## .semanrc
When initially starting the `seman server-start`, the `.semanrc` config file will be ran. This is interpretted as a list of SEMAN commands. SEMAN will attempt to find this file in the following locations.

```sh
## SEMANRC env variable
$SEMANRC

## XDG HOME
${XDG_CONFIG_HOME}/.config/seman/.semanrc

## FALLBACK HOME
${HOME}/.config/seman/.semanrc
```

An example config.

```sh
defservice-start picom picom
defservice-start nm-applet nm-applet
defservice-start dunst dunst

timer _ 10min "echo \"henlo word!\""

exec some-script
```
