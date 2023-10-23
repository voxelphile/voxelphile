<script>
	import { enhance } from "$app/forms";
    import "../../../form.postcss";
    import "./settings.postcss";
    import { get, writable } from 'svelte/store';
    import { profile_data } from "../store";
    export let form;

    /** @type {import('./$types').PageData} */
	export let data;

    let needs_to_be_committed = writable(false);

    async function load_profile(image_file) {
        return new Promise((resolve, reject) => {
            var reader = new FileReader();

            reader.onload = (event) => {
                if (event.target == null) {
                    return;
                }
                var data = event.target.result;
                resolve(data);
            };
            reader.readAsDataURL(image_file);
        });
    }
	
	async function change_profile(e) {
        let image_file = e.target.files[0];
        let raw_profile_data = await load_profile(image_file);
        let processed_profile_data = await resize_profile(raw_profile_data);
        if (processed_profile_data == null) {
            return;
        }
        profile_data.set(processed_profile_data);
        mark_submit_error();
    }

    const profile_image_size = 120;

    async function draw_profile(image, canvas_context) {
        return new Promise(resolve =>  {
            image.addEventListener('load', function() {
                if (canvas_context == null) {
                    return null;
                }
                canvas_context.drawImage(image, 0, 0, profile_image_size, profile_image_size);
                resolve(undefined);
            }, false);
        });
    }

    async function resize_profile(raw_image_url) {
        var image = new Image();
        image.src = raw_image_url;
        var canvas = document.createElement('canvas'),
            width = profile_image_size,
            height = profile_image_size;
        canvas.width = width;
        canvas.height = height;
        let canvas_context = canvas.getContext('2d');
        await draw_profile(image, canvas_context);
        var processed_image_url = canvas.toDataURL('image/jpeg');
        return processed_image_url;
    }

    const click_profile = () => {
        let profile_file = document.getElementById('profile-file');
        if(profile_file == null) {
            return;
        }
        profile_file.click();
    };
    const enhance_form = (e) => {
        if(!$needs_to_be_committed) {
            return e.cancel();
        }
        
        e.formData.set("profile", $profile_data);

        return async ({ update }) => { 
            await update({ reset: false });

            hide_submit_error();
        };
    };
    function mark_submit_error() {
        needs_to_be_committed.set(true);
        document.getElementsByClassName('submit-error')[0].className = 'submit-error';
    }
    
    function hide_submit_error() {
        needs_to_be_committed.set(false);
        document.getElementsByClassName('submit-error')[0].className = 'submit-error submit-error-invisible';
    }
</script>
<header>
    <title>Settings │ Dashboard │ Voxelphile</title>
</header>
<form id = "form" method = "POST" enctype="multipart/form-data" use:enhance={enhance_form}>
    <div id = "upload-image">
        {#if $profile_data}
            <img class = "profile-image" src = {$profile_data} draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
        {:else if data.profile_url}
            <img class = "profile-image" style="transform: scale(80%);" src = {data.profile_url} draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
        {:else}
            <img class = "profile-image" style="filter: invert(96%) sepia(0%) saturate(35%) hue-rotate(221deg) brightness(98%) contrast(84%); transform: scale(80%);" src = "/default-profile.png" draggable="false" on:dragstart={(e) => { e.preventDefault() }}/>
        {/if}
        <input id = "profile-file" type="file" name="profile" on:change={(e)=>change_profile(e)} accept="image/*">
        <div id = "profile-button-container"></div>
        <input type="button" id="profile-button" value="Choose Profile Picture" on:click={(e) => click_profile()}/>
    </div>
    <noscript>
        <style>
            #upload-image {
                display: none !important;
            }
        </style>
    </noscript>
    <div class = "input-group">
        <div class = "label">
            <label class = "label-text">Email</label>
            <label class = "label-text red error"></label>
        </div>
        <input class = "input user" type = "email" name = "email" value = {data.email} on:input={(e) => mark_submit_error()}/>
    </div>
    <div class = "input-group">
        <div class = "label">
            <label class = "label-text">Username</label>
            {#if form?.username_error} 
                <label class = "label-text red error">{form?.username_error}</label>
            {:else}
                <label class = "label-text red error"></label>
            {/if}
        </div>
        <input class = "input user" name = "username" value = {data.username} on:input={(e) => mark_submit_error()}/>
    </div>
    <div class = "input-group">
        <div class = "label">
            <label class = "label-text">Password</label>
            {#if form?.password_error} 
                <label class = "label-text red error">{form?.password_error}</label>
            {:else}
                <label class = "label-text red error"></label>
            {/if}
        </div>
        <input class = "input user" type = "password" name = "password" on:input={(e) => mark_submit_error()}/>
    </div>
    <div class = "input-group">
        <div class = "label">
            <label class = "label-text">Reenter Password</label>
            {#if form?.repassword_error} 
                <label class = "label-text red error">{form?.repassword_error}</label>
            {:else}
                <label class = "label-text red error"></label>
            {/if}
        </div>
        <input class = "input user" type = "password" name = "repassword" on:input={(e) => mark_submit_error()}/>
    </div>
    <button disabled = '{!$needs_to_be_committed}' id = "submit" class = "white submit">Commit</button>
    <p class = "submit-error submit-error-invisible">You have changes that need to be committed.</p>
</form>