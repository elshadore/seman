# SEMAN: SErvice MANager
SEMAN is a simple linux service manager designed for usage with simple (tiling) window managers. It allows for the registration, creation, deletion and the ability to restart shell processes created, and has a simple timer functionality as well. It has a simple server client model with the `seman-server` binary being the server and the `seman` binary being the client that passes messages to the server.


## Seman Commands
Below is a listing of all the commands the seman server interacts with.

Here is an example of the shell interface.
```sh
seman timer foo 10min "echo \"henlo word!\""
```

### `-h` || `help`
Displays the help menu.

### `defservice`
Define a simple service. Service names are unique, defining a service with the same name will overwrite the previous service.
- Argument 1: service name
- Argument 2: service command

### `services`
Provides a listing of all services, and wether they are active or not.

### `service-start`
Start/restart a named service.
- Argument 1: service name.

### `service-stop`
Stops a service killing the process.

### `defservice-start`
Combines `defservice` and `service-start`.
- Argument 1: service name
- Argument 2: service command

### `timer`
Creates a named timer. Timers can have the same name and will not override each other unlike services.
- Argument 1: timer name
- Argument 2: time
- Argument 3: command ran on completion

### `timers`
Returns a list of all currently active timers.

### `timer-kill`
Kills a timer(s) by name. If there are timers with the same name it kills them all.

- Argument 1: timer name

## .semanrc
When initially starting the `seman-server`, the `.semanrc` config file will be ran. This is interpretted as a list of seman commands.

An example config.

```.semanrc
defservice-start picom picom
defservice-start nm-applet nm-applet
defservice-start dunst dunst

timer _ 10min "echo \"henlo word!\""

exec some-script
```
