<script>
    import "../../app.postcss";
    import "./login-register.postcss";
    import "../../form.postcss";
</script>

<div id = "container">
    <div id = "form-container">
        <div id = "logo">
            <a id = "link" href = "/">
                <img class = "picture" src = "/logo.png" draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
            </a>
        </div>
        <form id = "form" method = "POST">
            <main id = "contents">
                <slot />
            </main>
        </form>
    </div>
</div>