<script>
    import "../app.postcss";
    import "./root.postcss";

	import { page } from '$app/stores';

    function get_error_info() {
        if(page == null || $page.error == null) {
            return '';
        }
        var message;
        switch($page.status) {
            case 400: message = "Bad Request"
            break;
            case 401: message = "Unauthorized"
            break;
            case 402: message = "Payment Required"
            break;
            case 403: message = "Forbidden"
            break;
            case 404: message = "Not Found"
            break;
            case 422: message = "Unprocessable Entity"
            break;
            case 500: message = "Internal Server Error"
            break;
            case 501: message = "Not Implemented"
            break;
            case 503: message = "Service Unavailable"
            break;
            default: message = "Unknown"
        };
        return message;
    }
</script>

<div id = "container">
    <div id = "info-container">
        <header id = "header">
            <div id = "logo">
                <img class = "picture" src = "/logo.png" draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
            </div>
            <p id = "voxelphile">Voxelphile</p>
        </header>
        
        {#if $page.status != 404}
        <div id = "user">
            <button id = "submit" style="width: 100%" class = "white submit refresh" on:click={(e) => { window.location.reload() }}>Go back</button>
        </div>
        {/if}
        <noscript>
            <style>
                .refresh = {
                    display: none;
                }
            </style>
        </noscript>
        <div id = "user">
            <a id = "submit" style="width: 100%" class = "white submit" href = "/">Home</a>
        </div>
        <p class = "red failed">{get_error_info()}</p>
    </div>
</div>