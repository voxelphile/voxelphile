<script>
    import "../../app.postcss";
    import "../../login-register.postcss";
    import "../../form.postcss";
</script>

<div id = "container">
    <div id = "form-container">
        <div id = "logo">
            <img class = "picture" src = "logo.png"/>
        </div>
        <form id = "form" method = "POST">
            <input id = "user" class = "input" type = "email" name = "email" placeholder="Email"/>
            <input id = "user" class = "input" name = "username" placeholder="Username"/>
            <input id = "user" class = "input" type = "password" name = "password" placeholder="Password"/>
            <button id = "submit" type = "submit" class = "white input">Signup</button>
            <a id = "submit" class = "black input" href = "/login">Already have an account?</a>
        </form>
    </div>
</div>