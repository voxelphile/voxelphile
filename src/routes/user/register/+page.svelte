<script>
	import { writable } from 'svelte/store';
	import { disabled } from '../store.js';
	import { onMount } from 'svelte';

    export let form;
    
    let username_empty = writable(true);
    let email_empty = writable(true);
    let password_empty = writable(true);
    let repassword_empty = writable(true);

    onMount(() => {
        disabled.set(false);
    })

</script>
<div class = "input-group">
    <div class = "label">
        <label class = "label-text">Email</label>
        {#if form?.email_error} 
                <label class = "label-text red error">{form?.email_error}</label>
            {:else}
                <label class = "label-text red error"></label>
            {/if}
    </div>
    <input id = "user" class = "input" type = "email" name = "email"  on:input={(e) => {username_empty.set(e?.target?.value == '')}}/>
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
    <input id = "user" class = "input" name = "username"  on:input={(e) => {email_empty.set(e?.target?.value == '')}}/>
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
    <input id = "user" class = "input" type = "password" name = "password"  on:input={(e) => {password_empty.set(e?.target?.value == '')}}/>
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
    <input id = "user" class = "input" type = "password" name = "repassword"  on:input={(e) => {repassword_empty.set(e?.target?.value == '')}}/>
</div>
<button disabled = { $username_empty || $email_empty || $password_empty || $repassword_empty || $disabled } type = "submit" class = "white submit">Signup</button>
<a class = "black submit" href = "/user/login">Already have an account?</a>