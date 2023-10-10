<script>
    import "../../app.postcss";
    import "./dashboard.postcss";
    import "./store.js";
    import { get } from 'svelte/store';
	import { profile_data } from "./store.js";
	import { onMount } from "svelte";

    /** @type {import('./$types').LayoutData} */
    export let data;
</script>

<div id = "container">
    <header id = "header">
        <div id = "logo">
            <img class = "picture" src = "/logo.png" draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
        </div>
        <div id = "center">
        </div>
        <div id = "profile-container">
            <div id = "profile">
                {#if $profile_data}
                    <img class = "profile-image" src = {$profile_data} draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
                {:else if data.profile_url}
                    <img class = "profile-image" src = {data.profile_url} draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
                {:else}
                    <img class = "picture" style = "filter: invert(96%) sepia(0%) saturate(35%) hue-rotate(221deg) brightness(98%) contrast(84%);" src = "/default-profile.png" draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
                {/if}
            </div>
        </div>
    </header>
    <div id = "body">
        <div id = "menu">
            <a href="/dashboard" class = "link">Home</a>
            <br/>
            <a href="/dashboard/settings" class = "link">Settings</a>
            <br/>
            <form method = "POST" action="/user/logout">
                <button class = "link">Logout</button>
            </form>
        </div>
        <main id = "contents">
            <slot />
        </main>
    </div>
</div>