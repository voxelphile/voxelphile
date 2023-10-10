<script>
    import "../../app.postcss";
    import "../root.postcss";
    import "./voxelphile-plus.postcss";
</script>

<div id = "container">
    <div id = "info-container">
        <header id = "header" style="align-items: center;">
            <div id = "logo" class = "voxelphile-plus-logo">
                <img class = "picture" src = "/logo.png" draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
            </div>
            <p id = "voxelphile" class = "voxelphile-plus">Voxelphile+</p>
            <div class = "explanation">

                <p>My monetization strategy is in the works.</p>
                <p>The plan is a monthly subscription for cosmetics</p>
                <p>or an additional payment for chunk loaders.</p>
                <p>Payment will be accessible through Stripe or Patreon (your choice).</p>
                <p>Stand by until the platform is at a place where I can begin accepting payment!</p>
            
            </div>
            <div id = "user">
                <a id = "submit" style="width: 100%" class = "white submit" href = "/">Home</a>
            </div>
            <style>
                .explanation {
                    text-align: center;
                }
            </style>
        </header>
        
    </div>
</div>