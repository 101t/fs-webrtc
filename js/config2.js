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
        console.log('Error attaching stream to element. ##################');
    }
    //console.log(element);
    setTimeout(function() {
        element.play();
    }, 0);
}

function createSimpleSIP(mymail, username, password, displayName, targetmail){
    //navigator.getUserMedia = navigator.getUserMedia || navigator.webkitGetUserMedia || navigator.mozGetUserMedia;
    navigator.mediaDevices.getUserMedia({ audio: true, video: true}).then(function(stream){
        window.localStream = stream;
        var options = {
            /*constraints: {
                audio: true,
                video: true
            },*/
            media: {stream: window.localStream},
            sessionDescriptionHandlerOptions: {
                constraints: {
                  audio: true,
                  video: true
                },
                //iceCheckingTimeout: 5000
            }
        };
        //attachMediaStream(localVideo, stream);
        localVideo.srcObject = stream;
        localVideo.play();
        // set up our private UA with private URI
        priv = new SIP.UA({
            uri: mymail,
            wsServers: url,
            authorizationUser: username,
            password: password,
            displayName: displayName,
            autostart: true,
            register: true,
            traceSip: true,
            transportOptions: { wsServers: url }
            /*sessionDescriptionHandlerFactoryOptions: {
                constraints: {
                    audio: true,
                    video: true
                },
                rtcConfiguration: {
                    rtcpMuxPolicy: 'negotiate'
                },
            }*/
        });
        priv.once('registered', function(){
            //priv.message(conf.configuation.uri, "Invite me, please!");
        });
        priv.on('invite', function(session){
            console.log('accepting session ##################');
            session.accept(options);
            session.on('accepted', function(){
                console.log(this);
                //this.data.remote = attachMediaStream(remoteVideo, session.sessionDescriptionHandler.peerConnection.getRemoteStreams()[0]);
                mute({
                    audio: toggles['audio'].classList.contains('off'),
                    video: toggles['video'].classList.contains('off')
                });
                //this.data.remote;
                remoteVideo.srcObject = session.sessionDescriptionHandler.peerConnection.getRemoteStreams()[0];
                remoteVideo.play();
            });
            session.on('bye', function(){
                console.log('Bye')
                //console.log(this.data.remote);
            });
            session.on('trackAdded', function(){
                console.log("trackAdded ##################");
                var pc = session.sessionDescriptionHandler.peerConnection;
                var remoteStream = new MediaStream();
                pc.getReceivers().forEach(function(receiver) {
                    remoteStream.addTrack(receiver.track);
                });
                //attachMediaStream(remoteVideo, remoteStream);
                remoteVideo.srcObject = remoteStream;
                remoteVideo.play();

                var localStream = stream;
                pc.getSenders().forEach(function(sender) {
                    localStream.addTrack(sender.track);
                });
            });
        });
        priv.start();
        priv.invite(targetmail, options)
        priv.on('accepted', function(){
            //this.data.remote = attachMediaStream(remoteVideo, session.sessionDescriptionHandler.peerConnection.getRemoteStreams()[0]);
            mute({
                audio: toggles['audio'].classList.contains('off'),
                video: toggles['video'].classList.contains('off') 
            });
            remoteVideo.srcObject = session.sessionDescriptionHandler.peerConnection.getRemoteStreams()[0];
            remoteVideo.play();
        });
        priv.on('bye', function(){
            console.log('Bye!!!!');
            console.log(this.data.remote);
        });
        // set up our shared uri used to receive join requests (and other things)
        /*conf = new SIP.UA({
            uri: targetmail,
            wsServers: url,
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
        conf.start();*/
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
}