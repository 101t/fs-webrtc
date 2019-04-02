<div class="card" >
    <div id="remote">
        <video id="remotevideo" class="card-img-top" autoplay></video>
    </div>
    <div class="card-body">
        <div class="row">
            <div class="col-md-6">
                <div id="local">
                    <video id="localvideo" muted style="width:100%;" autoplay></video>
                    <button id="audio-toggle" class="btn btn-secondary on" type="button">Toggle Audio</button>
                    <button id="video-toggle" class="btn btn-secondary on" type="button">Toggle Video</button>
                </div>
            </div>
            <div class="col-md-6">
                <div class="button-group btn-group-sm float-right" style="display:none;">
                    <button class="btn btn-primary" id="callButton" type="button">Start Call <i class="fas fa-phone"></i></button>
                    <button class="btn btn-danger" id="cancelButton" type="button">Cancel Call <i class="fas fa-phone-slash"></i></button>
                    <button class="btn btn-success" id="answerButton" type="button">Answer Call <i class="fas fa-volume-up"></i></button>
                </div>
            </div>
        </div>
    </div>
</div>