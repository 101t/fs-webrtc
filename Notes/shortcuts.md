### After installing FreeSWITCH:

You could run freeswitch as service in background
```sh
sudo /usr/local/freeswitch/bin/freeswitch -nc
```

You may run freeswitch cli
```sh
/usr/local/freeswitch/bin/fs_cli
```

### Some usefull commands:

To see all registered profile internal users

```
list_users
```
To see all active profile profile user

```
sofia status profile internal reg
```

To load module or codec
```
load mod_mp4v
```

To show all installed module or codec
```
show codec
show module
```

To reload all xml after configure it
```
reloadxml
```

### Important URL and Article References:

1. http://omid-mohajerani.blogspot.com/2014/07/learning-freeswitch-as-asterisk-man.html

2. https://saevolgo.blogspot.com/2012/07/freeswitch-with-sip-users-in-mysql-mod.html

3. https://freeswitch.org/confluence/display/FREESWITCH/Command-Line+Interface+fs_cli