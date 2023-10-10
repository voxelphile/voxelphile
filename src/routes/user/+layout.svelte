<script>
    import "../../app.postcss";
    import "./login-register.postcss";
    import "../../form.postcss";
	import { enhance } from "$app/forms";
	import { disabled } from "./store";
    
    function prevent_clear() {
        disabled.set(true);
        return async ({ update }) => { 
            await update({ reset: false });
            disabled.set(false);
        };
    }
</script>

<div id = "container">
    <div id = "form-container">
        <div id = "logo">
            <a id = "link" href = "/">
                <img class = "picture" src = "/logo.png" draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
            </a>
        </div>
        <form id = "form" method = "POST" use:enhance={prevent_clear}>
            <main id = "contents">
                <slot />
            </main>
        </form>
    </div>
</div>