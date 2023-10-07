<script>
    import "../../app.postcss";
    import "../root.postcss";
    import "./voxelphile-plus.postcss";
</script>

<div id = "container">
    <div id = "info-container">
        <header id = "header">
            <div id = "logo" class = "voxelphile-plus-logo">
                <img class = "picture" src = "/logo.png" draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
            </div>
            <p id = "voxelphile" class = "voxelphile-plus">Voxelphile+</p>
        </header>
        
    </div>
</div>