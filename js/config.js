var host = "192.168.1.170";
var port = "5066";
var url = `ws://${host}:${port}/`;

var videoMedia = true,
    audioMedia = true;
var mediaConstraints = {
    audio: audioMedia,
    video: videoMedia
};

var priv, session;

var localVideo = document.getElementById("localvideo");
var remoteVideo = document.getElementById("remotevideo");

var callstatus = document.getElementById('callstatus');

var media = { remote: { audio: remoteVideo, video: remoteVideo }, local: { audio: localVideo, video: localVideo } };

function createSimpleSIP(mymail, username, password, displayName, targetmail){
	var localStream = null;
	if (window.RTCPeerConnection) {
		priv = new SIP.Web.Simple({
			ua: {
				uri: mymail,
				wsServers: url,
				authorizationUser: username,
				password: password,
				displayName: displayName,
				register: true,
				traceSip: true,
				hackViaTcp: true,
				transportOptions: { wsServers: url },
				sessionDescriptionHandlerFactoryOptions: {
					constraints: {
						audio: true,
						video: true
					},
					rtcConfiguration: {
						rtcpMuxPolicy: 'negotiate'
					},
				}
			},
			media: media
		});
		function addListeners(){
			
			session.on('progress', function(){
				callstatus.innerHTML = "Connecting...";
			});
			session.on('accepted', function(){
				callstatus.innerHTML = "Connected!";
			});
			session.on('failed', function(){
				callstatus.innerHTML = "Call failed. Try again?";
			});
			session.on('bye', function(){
				if (session === this) {
					callstatus.innerHTML = "Bye! Invite again?";
					session = null;
				}
			}.bind(session));
			session.on('refer', function(request, newSession){
				callstatus.innerHTML = "Ringing...";
				session = newSession;
				addListeners();
			});
		}
		priv.on('ringing', function(res_session){
			session = res_session
			addListeners();
			priv.answer();
		});
		priv.on('registered', function(){
			session = priv.call(targetmail);
			if (!!session) {
				addListeners();
			} else {
				callstatus.innerHTML = "No session created!";
			}
		});
	}
}