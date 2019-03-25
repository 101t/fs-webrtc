<!DOCTYPE html>
<html>
<head>
  <title>WS Test Connection | SIP JS</title>
</head>
<body>
<ul>
  <li><a href="ws_test.php">WS Test</a></li>
  <li><a href="wss_test.php">WSS Test</a></li>
</ul>
<script type="text/javascript" src="js/sip-0.13.5.min.js"></script>
<script type="text/javascript">
var config = {
  // Replace this IP address with your FreeSWITCH IP address
  uri: '1000@192.168.1.170',

  // Replace this IP address with your FreeSWITCH IP address
  // and replace the port with your FreeSWITCH ws port
  //ws_servers: 'ws://192.168.1.170:5066',
  transportOptions: {
    wsServers: ['ws://192.168.1.170:5066']
  },

  // FreeSWITCH Default Username
  authorizationUser: '1000',
  displayName: "Tarek Kalaji",
  autostart: true,

  // FreeSWITCH Default Password
  password: '1234',
  register: true
};

window.ua = new SIP.UA(config);
ua.start();
console.log(ua.isRegistered());
</script>
</body>
</html>