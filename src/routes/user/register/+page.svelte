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
    <input id = "user" class = "input" name = "email"/>
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
    <input id = "user" class = "input" type = "password" name = "username"/>
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
    <input id = "user" class = "input" name = "password"/>
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
    <input id = "user" class = "input" type = "password" name = "repassword"/>
</div>
<button type = "submit" class = "white submit">Signup</button>
<a class = "black submit" href = "/user/login">Already have an account?</a>