<?php 
$username = '1001';
$password = '1234';
$displayName = 'Bob';
$targetuser = '1000';
function dd($name){echo $name;}
?>
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title><?php dd($displayName) ?> - Test Connection | SIP JS</title>
    <?php include("includes/css.php"); ?>
</head>
<body>
<?php include("includes/menu.php"); ?>
<div class="container">
    <h3><?php dd($displayName) ?> Test Connection</h3>
    <?php include("includes/content.php") ?>
</div>
<?php include("includes/js.php"); ?>
<script type="text/javascript" src="js/config.js"></script>
<script type="text/javascript">
var targetuser = '<?php dd($targetuser) ?>';
var targetmail = `${targetuser}@${host}`;
var username = "<?php dd($username) ?>";
var password = "<?php dd($password) ?>";
var displayName = "<?php dd($displayName) ?>";
var mymail = `${username}@${host}`;

createSimpleSIP(mymail, username, password, displayName, targetmail);
//createSimpleSIP(targetmail, username, password, displayName, mymail);
</script>
</body>
</html>