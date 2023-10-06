<script>
    import "../../../form.postcss";
    import "./settings.postcss";
</script>

<form id = "form" method = "POST">
    <div id = "upload-image">
        <img id="image" src="/profile.jpg" alt="your image" />
        <input id = "profile" type="file" name="img" accept="image/*">
    </div>
    <input id = "user" class = "input" type = "email" name = "identifier" placeholder="Email"/>
    <input id = "user" class = "input" name = "username" placeholder="Username"/>
    <input id = "user" class = "input" type = "password" name = "password" placeholder="Password"/>
    <input id = "user" class = "input" type = "password" name = "password" placeholder="Re-enter Password"/>
    <button id = "submit" type = "submit" class = "white input">Change</button>
</form>