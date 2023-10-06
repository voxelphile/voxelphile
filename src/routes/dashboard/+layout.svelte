<script>
    import "../../app.postcss";
    import "./dashboard.postcss";
</script>

<div id = "container">
    <header id = "header">
        <div id = "logo">
            <img class = "picture" src = "/logo.png"/>
        </div>
        <div id = "center">
        </div>
        <div id = "profile-container">
            <div id = "profile">
                <img class = "picture" src = "/profile.jpg"/>
            </div>
        </div>
    </header>
    <body id = "body">
        <div id = "menu">
            <a href="/dashboard" class = "link">Home</a>
            <br/>
            <a href="/dashboard/settings" class = "link">Settings</a>
        </div>
        <main id = "contents">
            <slot />
        </main>
    </body>
</div>