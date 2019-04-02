var host = "192.168.1.170";
var port = "5066";
var url = `ws://${host}:${port}`;

var videoMedia = true,
    audioMedia = true;
var baseSession = null;
var simpleUA = null;
var mediaConstraints = {
    audio: audioMedia,
    video: videoMedia
};

var conf, priv;

/*var localVideo = document.querySelector("#local video");
var remoteVideo = document.querySelector("#remote video");*/
var localVideo = document.getElementById("localvideo");
var remoteVideo = document.getElementById("remotevideo");

var toggles = {
	audio: document.getElementById("audio-toggle"),
    video: document.getElementById("video-toggle")
};

function createSimpleSIP(mymail, username, password, displayName, targetmail){
	var localStream = null;
	/*navigator.mediaDevices.getUserMedia({ audio: true, video: true}).then(function(stream){
		localStream = stream;
		localVideo.srcObject = stream;
        localVideo.play();

	}, function() {});*/
	if (window.RTCPeerConnection) {
		priv = new SIP.Web.Simple({
			media: {
				remote: {
					video: remoteVideo,
					audio: remoteVideo
				},
				local: {
					video: localVideo,
					audio: localVideo
				}
			},
			ua: {
				uri: mymail,
		        wsServers: url,
		        authorizationUser: username,
		        password: password,
		        displayName: displayName,
		        //rel100: SIP.C.supported.SUPPORTED,
		        hackViaTcp: true,
		        autostart: false,
		        register: true,
		        traceSip: true,
		        sessionDescriptionHandlerFactoryOptions: {
		            constraints: {
		                audio: true,
		                video: true
		            },
		            rtcConfiguration: {
		                rtcpMuxPolicy: 'negotiate'
		            },
		        }
			}
		});
		/*priv = new SIP.UA({
			uri: mymail,
			wsServers: url,
			authorizationUser: username,
			password: password,
			displayName: displayName,
			hackViaTcp: true,
			autostart: true,
			register: true,
			traceSip: true,
			transportOptions: { wsServers: url },
			sessionDescriptionHandlerFactoryOptions: {
				constraints: mediaConstraints,
				render: {
					remote: remoteVideo,
					local: localVideo
				},
				rtcConfiguration: {
				    rtcpMuxPolicy: 'negotiate'
				},
				alwaysAcquireMediaFirst: true,
			}
		});*/
		priv.on('ended', function(){
			console.log("ended");
		});
		priv.on('connected', function(session){
			console.log("connected");
			/*var pc = session.sessionDescriptionHandler.peerConnection;
			var receivers = pc.getReceivers();
			if (receivers.length) {
				var remoteStream = new MediaStream();
				receivers.forEach(function(receiver){
					remoteStream.addTrack(receiver.track);
				});
				console.warn("HELLO :D :D :D :D");
				remoteVideo.srcObject = remoteStream;
				remoteVideo.play();
			}*/
		})
		priv.on('ringing', function(){
			priv.answer();
		});
		/*if (priv.state === SIP.Web.Simple.C.STATUS_NULL || 
			priv.state === SIP.Web.Simple.C.STATUS_COMPLETED) {
			priv.call(targetmail);
		} else {
			priv.hangup();
		}*/
		priv.on('registered', function(session){
			priv.call(targetmail, {
				media: {
					constraints: mediaConstraints,
					render: {
						remote: remoteVideo,
						local: localVideo,
					}
				},
			});
			/*priv.hold();
			priv.unhold();*/
		});
		priv.on('invite', function(session){
			session.accept({
				constraints: mediaConstraints,
				sessionDescriptionHandlerFactoryOptions: {
					peerConnectionOptions: {
						rtcConfiguration: {
							iceTransportPolicy: 'all',
							iceServers: [{
								urls: 'stun:stun.l.google.com:19302'
							}],
						}
					}
				},
				media: {
					constraints: mediaConstraints,
					render: {
						remote: remoteVideo,
						//local: localVideo,
					},
				}
			});
			session.on('SessionDescriptionHandler-created', function(sdh){
				sdh.on('userMedia', function(stream){
					localVideo.srcObject = new MediaStream(stream);
					localVideo.play();
				})
			})
		});
		priv.on('accepted', function(session){
			console.log("session accepted");
			var pc = session.sessionDescriptionHandler.peerConnection;
			var receivers = pc.getReceivers();
			if (receivers.length) {
				var remoteStream = new MediaStream();
				receivers.forEach(function(receiver){
					remoteStream.addTrack(receiver.track);
				});
				remoteVideo.srcObject = remoteStream;
				remoteVideo.play();
			};
			session.on('trackAdded', function(){
				var pc = session.sessionDescriptionHandler.peerConnection;
				var receivers = pc.getReceivers();
				if (receivers.length) {
					var remoteStream = new MediaStream();
					receivers.forEach(function(receiver){
						remoteStream.addTrack(receiver.track);
					});
					console.warn("HELLO :D :D :D :D");
					remoteVideo.srcObject = remoteStream;
					remoteVideo.play();
				}
				var senders = pc.getSenders();
				if (senders.length) {
					var localStream = new MediaStream();
					senders.forEach(function(sender){
						localStream.addTrack(sender.track);
					});
					localVideo.srcObject = localStream;
					localVideo.play();
				}
			});
		});
		/*priv.invite(targetmail, {
			sessionDescriptionHandlerOptions: {
				constraints: {
					audio: true,
					video: true
				},
				iceCheckingTimeout: 5000
			}
		});*/
		priv.on('userMedia', function (stream) {
			console.log(stream);
		})
		/*var session = priv.subscribe(targetmail, 'presence');
		session.invite(targetmail, {
			sessionDescriptionHandlerOptions: {
				constraints: {
					audio: true,
					video: false
				},
				iceCheckingTimeout: 5000
			}
		});*/
	}
}