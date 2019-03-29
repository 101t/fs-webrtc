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

var localVideo = document.querySelector("#local video");
var remoteVideo = document.querySelector("#remote video");

var toggles = {
	audio: document.getElementById("audio-toggle"),
    video: document.getElementById("video-toggle")
};

var room = location.hash && location.hash.replace(/^#([a-zA-Z0-9-_.~~*'()&=+$,;?\/%]*).*/, '$1');

function attachMediaStream(element, stream) {
    if (typeof element.srcObject !== 'undefined') {
        element.srcObject = stream;
    } else if (typeof element.mozSrcObject !== 'undefined') {
        element.mozSrcObject = stream;
    } else if (typeof element.src !== 'undefined') {
        element.src = URL.createObjectURL(stream);
    } else {
        console.log('Error attaching stream to element.');
    }
    //console.log(element);
    setTimeout(function() {
        element.play();
    }, 0);
}

function createSimpleSIP(mymail, username, password, displayName, targetmail){
	//SIP.WebRTC.isSupported();
	//window.RTCPeerConnection
	//navigator.getUserMedia = navigator.getUserMedia || navigator.webkitGetUserMedia || navigator.mozGetUserMedia;
	//navigator.mediaDevices.getUserMedia
	navigator.mediaDevices.getUserMedia({ audio: true, video: true}, function(stream){
		console.log("get localStream");
		window.localStream = stream;
		var options = {
			media: {stream: window.localStream}
		};
		attachMediaStream(localVideo, stream);
		// set up our private UA with private URI
		priv = new SIP.UA({
			uri: mymail,
			wsServers: [url],
			authorizationUser: username,
			password: password,
			displayName: displayName,
			autostart: true,
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
		});
		priv.once('registered', function(){
			//priv.message(conf.configuation.uri, "Invite me, please!");
		});
		priv.on('invite', function(session){
			console.log('accepting session');
			session.accept(options);
			session.on('accepted', function(){
				this.data.remote = attachMediaStream(remoteVideo, this.getRemoteStreams()[0]);
				this.mute({
					audio: toggles['audio'].classList.contains('off'),
					video: toggles['video'].classList.contains('off')
				});
				this.data.remote;
			});
			session.on('bye', function(){
				console.log('Bye')
				console.log(this.data.remote);
			});
		});
		priv.start();

		// set up our shared uri used to receive join requests (and other things)
		conf = new SIP.UA({
			uri: targetmail,
			wsServers: [url],
			traceSip: true,
		});
		conf.on('message', function(message){
			console.log("sending invite with localStream");
			console.log(window.localStream);
			priv.invite(message.remoteIdentity.uri.toString(), options)
			priv.on('accepted', function(){
				this.data.remote = attachMediaStream(remoteVideo, this.getRemoteStreams()[0]);
				this.mute({
					audio: toggles['audio'].classList.contains('off'),
					video: toggles['video'].classList.contains('off') 
				});
			});
			priv.on('bye', function(){
				console.log('Bye!!!!');
				console.log(this.data.remote);
			});
		});
		conf.start();
		// shut everything down cleanly before the window closes
		window.onbeforeunload = function() {
			//if (!!conf) {conf.stop();}
			if (!!priv) {priv.stop();}
		};
		function mute(type, display) {
		    return function(e) {
		        var toggle = toggles[type].classList.contains('off');

		        // Toggle classes
		        toggles[type].classList[toggle ? 'add' : 'remove']('on');
		        toggles[type].classList[toggle ? 'remove' : 'add']('off');

		        if (type === 'video') localVideo[toggle ? 'play' : 'pause']();

		        var s;
		        for (s in priv.sessions) {
		            priv.sessions[s][toggle ? 'unmute' : 'mute']({
		                video: type === 'video',
		                audio: type === 'audio'
		            });
		        }

		        toggles[type].blur();
		    };
		}
		toggles.video.addEventListener('click', mute('video', 'Video'), false);
		toggles.audio.addEventListener('click', mute('audio', 'Audio'), false);
	}, function() {});
	window.localStream = new MediaStream();
	var options = {
		media: {stream: window.localStream}
	};
	attachMediaStream(localVideo, window.localStream);
	
}