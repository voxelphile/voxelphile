import { error, fail } from "@sveltejs/kit";
import { fetch_promise } from "../../../user-form.js";
/** @type {import('./$types').Actions} */
import { api } from "../../../const.js";
export const actions = {
	default: async (event) => {
		const formData = await event.request.formData();
        const username = formData.get('username')?.toString();        
        const email = formData.get('email')?.toString();        
        const password = formData.get('password')?.toString();
        const repassword = formData.get('repassword').toString();

        let errors = { repassword_error: '', password_error: '', username_error: '' };

        {
            const username = formData.get('username')?.toString();        
            const email = formData.get('email')?.toString();        
            const password = formData.get('password')?.toString();
            const repassword = formData.get('repassword').toString();

            let errors = {};

            if(password != '' && repassword != '' && password != repassword) {
                errors["repassword_error"] = 'Must match';
            }
            if(password == null || password != null && password.toString().length < 6) {
                errors["password_error"] = 'Must be at least 6 characters';
            }
            if(password != null && password.toString().length > 128) {
                errors["password_error"]= 'Must be at most 128 characters';
            }
            if(username != null && !username.toString().match(/^[0-9a-zA-Z]+$/)) {
                errors["username_error"]= 'Must be alphanumeric';
            }
            if(username == null || username != null && username.toString() == '') {
                errors["username_error"]= 'Cannot be empty';
            }
            if(username != null && username.toString().length > 32) {
                errors["username_error"]= 'Must be at most 32 characters';
            }
            if(email == null || email != null && email.toString() == '') {
                errors["email_error"]= 'Cannot be empty';
            }
            if(email != null && email.indexOf("@") == -1) {
                errors["email_error"]= 'Must contain an @ symbol';
            }
            if(email != null && email.indexOf("@") == email.length - 1) {
                errors["email_error"]= 'Must contain a part after the @ symbol';
            }
            
            if (Object.keys(errors).length > 0) {
                return errors;
            }
        }

        if (username == undefined || password == undefined || email == undefined) {
            return { success: false };
        }

        let json = { username, details: { password, email } };

        const request = new Request(api + "/user/register", {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify(json),
        });

        let response = await fetch(request).catch((response) => {
                throw error(response?.status);
            });
        
        if (response?.status != 200) {
            throw error(response?.status);
        }

        let jwt = JSON.parse(await response?.text());

        event.cookies.set("jwt", jwt, { path: '/' });

        return { jwt };
	}
}; 