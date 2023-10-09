<script>
	import { writable } from 'svelte/store';
    export let form;
    
    let email_empty = writable(true);
    let password_empty = writable(true);
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
    <input class = "user input" name = "email" on:input={(e) => {email_empty.set(e?.target?.value == '');}}/>
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
    <input class = "user input" type = "password" name = "password" on:input={(e) => {password_empty.set(e?.target?.value == '');}}/>
</div>
<button disabled = { $email_empty || $password_empty} id = "submit" type = "submit" class = "white submit">Login</button>
<a id = "submit" class = "black submit" href = "/user/register">Signup</a>
