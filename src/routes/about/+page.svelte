<script>
    import "../../app.postcss";
    import "../root.postcss";
    import "./voxelphile-plus.postcss";
</script>
<header>
    <title>About │ Voxelphile</title>
</header>
<div id = "container">
    <div id = "info-container">
        <header id = "header" style="align-items: center;">
            <div id = "logo" class = "voxelphile-logo">
                <img class = "picture" src = "/logo.png" draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
            </div>
            <p id = "voxelphile" class = "voxelphile">Voxelphile</p>
            <div class = "explanation">
                <p>Voxelphile is a content creation and game development company empowering people to play, create, and socialize within a voxel environment; players are free to do as they please in a sandbox-like environment.</p>
                <div class = "section">
                    <p>I am currently working on Xenotech, a Web3 Voxel MMO.</p>
                    <p>Xenotech is a technology-focused space-based open world sandbox where the user discovers an alien technology.</p>
                    <p>The player then uses this technology to enhance their builds and creations, which allows them to discover more, and so on and so forth...</p>
                    <p>Players will be able to gather material that is not ephermeral, each item will have a unique identifier that can be verified via a public blockchain. This means you might find your favorite sword was once possessed by your favorite content creator (and the like!)</p>
                </div>
                <div class = "section about-pictures">
                    <p>Here is some of my voxel work thus far:</p>
                    <img src = "/about-0.png" draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
                    <img src = "/about-1.png" draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
                    <img src = "/about-2.png" draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
                    <img src = "/about-3.png" draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
                    <img src = "/about-4.png" draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
                    <img src = "/about-6.png" draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
                    <div class = about-pictures-row>
                        <img src = "/about-5-1.png" draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
                        <img src = "/about-5-2.png" draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
                    </div>
                </div>
            </div>
            <div id = "user">
                <a id = "submit" style="width: 100%" class = "white submit" href = "/">Home</a>
            </div>
            <style>
                .explanation {
                    text-align: center;
                }
                .section {
                    margin-bottom: 20px;
                }
                .section > * {
                    margin: 0;
                }
                .about-pictures > * {
                    width: 40%;
                    margin: 5px;
                   
                }
                .about-pictures > p {
                    width: 100%;
                    text-align: clearInterval;
                }
                .about-pictures-row {
                    width: 100%;
                    display: flex;
                    flex-direction: row;
                    justify-content: center;
                }
                .about-pictures-row > * {
                    margin: 5px;
                    width: 40%;
                }
            </style>
        </header>
        
    </div>
</div>