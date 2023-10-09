<script>
    export let form;
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
    <input class = "user input" name = "email"/>
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
    <input class = "user input" type = "password" name = "password"/>
</div>
<button id = "submit" type = "submit" class = "white submit">Login</button>
<a id = "submit" class = "black submit" href = "/user/register">Signup</a>
