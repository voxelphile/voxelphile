/** @type {import('./$types').Actions} */
export const actions = {
	default: async (event) => {
		const formData = await event.request.formData();
        const identifier = formData.get('identifier')?.toString();        
        const password = formData.get('password')?.toString();

        if (identifier == undefined || password == undefined) {
            return { success: false };
        }

        let json = { password, details: {} };

        if (identifier.includes('@')) {
            json.details = { email: identifier };
        } else {
            json.details = { username: identifier };
        }

        const request = new Request("https://api.voxelphile.com/user/login", {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify(json),
        });

        fetch(request)
            .then((response) => {
                console.log(response);
            });
        
        return { success: true };
	}
};