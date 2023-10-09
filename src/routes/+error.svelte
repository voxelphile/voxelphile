<script>
    import "../app.postcss";
    import "./root.postcss";

	import { page } from '$app/stores';

    function get_error_info() {
        if(page == null || $page.error == null) {
            return '';
        }
        return $page.error.message;
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
        <div id = "user">
            <a id = "submit" style="width: 100%" class = "white submit" href = "/">Home</a>
        </div>
        <p class = "red failed">{get_error_info()}</p>
    </div>
</div>