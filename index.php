<!DOCTYPE html>
<html>
<head>
	<meta charset="utf-8">
	<title>SIP JS</title>
	<?php include("includes/css.php"); ?>
</head>
<body>
<?php include("includes/menu.php"); ?>
<div class="container">
	<div class="jumbotron text-center">
		<h1 class="display-4">Welcome to WebRTC app</h1>
		<p class="lead">In this app we used SIPjs as WebRTC-client and FreeSWITCH as WebRTC-server</p>
		<hr class="my-4">
		<div class="row">
			<div class="col-6">
				<img src="img/fs-logo.png" width="150">
			</div>
			<div class="col-6">
				<img src="img/sipjs-logo.png" width="150">
			</div>
		</div>
	</div>
</div>
<script type="text/javascript">
var menu = document.querySelectorAll("#menu li a");
menu[0].classList.add("active");
</script>
</body>
</html>