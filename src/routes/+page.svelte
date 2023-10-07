<script>
    import "../app.postcss";
    import "./root.postcss";
</script>

<div id = "container">
    <div id = "info-container">
        <header id = "header">
            <div id = "logo">
                <img class = "picture" src = "/logo.png" draggable="false" ondragstart="return false;"/>
            </div>
            <p id = "voxelphile">Welcome to Voxelphile!</p>
            <p id = "voxelphile" style = "font-size: 1.5em;">A new experience is under construction</p>
        </header>
        <div id = "user">
            <div class = "row">
                <a  class = "white submit" style = "width: 100%" href = "/user/login">Login</a>
            </div>
            <div class = "row">
                <a class = "white submit" style = "width: 100%"  href = "/user/register" >Signup</a>
            </div>
            <div></div>
            <div class = "row">
                <a class = "yellow black submit" style = "width: 100%" href = "/voxelphile-plus">Voxelphile+</a>
            </div>
        </div>
        <div id = "social">
            <a href="https://youtube.com/@voxelphile" class="social_media youtube"> </a>
			<a href="https://twitter.com/voxelphile" class="social_media twitter"> </a>
			<a href="https://instagram.com/voxelphile" class="social_media instagram"> </a>
			<a href="https://github.com/voxelphile" class="social_media github"> </a>
			<a href="https://discord.gg/VSZNs4AbK2" class="social_media discord"> </a>
        </div>
    </div>
</div>